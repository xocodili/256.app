/**
 * Three guided captures (neutral / smile / frown): 512×512 JPEGs.
 *
 * Face detection runs on-device only as a liveness/face-present gate (detector + landmarks). No
 * biometric template (face descriptor/embedding) is ever computed, sent, or stored — the server
 * generates an avatar from the photos transiently and keeps only the artwork.
 * Requires global `faceapi` from @vladmandic/face-api (loaded before bootstrap).
 */
(function () {
  "use strict";

  var MODEL_URL = "/static/vendor/face-api/model";
  var sharedModelsReady = false;
  var sharedModelsPromise = null;

  function faceApiGlobal() {
    if (typeof faceapi !== "undefined") return faceapi;
    if (typeof globalThis !== "undefined" && globalThis.faceapi) return globalThis.faceapi;
    if (typeof window !== "undefined" && window.faceapi) return window.faceapi;
    return undefined;
  }

  function preloadFaceCaptureModels() {
    if (sharedModelsReady) return Promise.resolve();
    if (sharedModelsPromise) return sharedModelsPromise;
    sharedModelsPromise = (async function () {
      var api = faceApiGlobal();
      if (!api) {
        throw new Error("face-api not loaded");
      }
      await api.nets.tinyFaceDetector.loadFromUri(MODEL_URL);
      await api.nets.faceLandmark68Net.loadFromUri(MODEL_URL);
      sharedModelsReady = true;
    })().catch(function (e) {
      sharedModelsPromise = null;
      throw e;
    });
    return sharedModelsPromise;
  }

  var TARGET = 512;
  var STABLE_FRAMES = 6;
  var MIN_FACE_HEIGHT_RATIO = 0.22;
  var MIN_CROP_SOURCE_SIDE = 200;
  var HEAD_TOP_PAD_RATIO = 0.78;
  var HEAD_BOTTOM_PAD_RATIO = 0.48;
  var HEAD_SIDE_PAD_RATIO = 0.42;
  var BEARD_EXTRA_BOTTOM_PAD_RATIO = 0.45;
  var MIN_FACE_LUMINANCE = 38;
  var LIGHT_SAMPLE_PX = 48;
  var CAPTURE_PAUSE_MS = 1000;
  var CAPTURE_STEPS = [
    { group: "neutral", promptKey: "stage_neutral" },
    { group: "smile", promptKey: "stage_smile" },
    { group: "frown", promptKey: "stage_frown" },
  ];

  function readUi(root) {
    var scope = root && root.querySelector ? root : document;
    var el = scope.querySelector("#face-capture-i18n") || document.getElementById("face-capture-i18n");
    if (!el) return {};
    try {
      return JSON.parse(el.textContent);
    } catch (_) {
      return {};
    }
  }

  function bootstrapFaceCapture(options) {
    options = options || {};
    var root = options.root || document;
    var shellEl = document.querySelector(".app-shell");
    var cardEl = root.querySelector
      ? root.querySelector(".face-capture-card")
      : document.querySelector(".face-capture-card");
    var godmotherCardEl = root.querySelector
      ? root.querySelector(".face-capture-godmother-card")
      : document.querySelector(".face-capture-godmother-card");
    var onboardingStep =
      shellEl && shellEl.getAttribute("data-onboarding-step")
        ? shellEl.getAttribute("data-onboarding-step").trim()
        : "";
    var faceCaptureCompleted =
      !!shellEl && shellEl.getAttribute("data-face-capture-ready") === "1";
    var profileReady = !!shellEl && shellEl.getAttribute("data-profile-ready") === "1";
    var godmotherOnly =
      onboardingStep === "godmother" ||
      (!!godmotherCardEl &&
        !godmotherCardEl.hasAttribute("hidden") &&
        faceCaptureCompleted &&
        profileReady &&
        (!cardEl || cardEl.hasAttribute("hidden")));

    if (godmotherOnly) {
      if (!godmotherCardEl || godmotherCardEl.getAttribute("data-godmother-bound") === "1") {
        return null;
      }
      godmotherCardEl.setAttribute("data-godmother-bound", "1");
    } else {
      var captureMode = cardEl
        ? (cardEl.getAttribute("data-face-capture-mode") || "onboarding").trim()
        : "onboarding";
      var redrawPanel = captureMode === "avatar_redraw";
      if (!cardEl) {
        return null;
      }
      if (cardEl.getAttribute("data-face-capture-bound") === "1" && !redrawPanel) {
        return null;
      }
      cardEl.setAttribute("data-face-capture-bound", "1");
    }

    var mode = cardEl
      ? (cardEl.getAttribute("data-face-capture-mode") || "onboarding").trim()
      : "onboarding";
    var isAvatarRedraw = mode === "avatar_redraw";
    var UI = readUi(root);
    var videoEl = root.querySelector("#face-video") || document.getElementById("face-video");
    var workCanvas = root.querySelector("#face-work-canvas") || document.getElementById("face-work-canvas");
    var statusEl = root.querySelector("#face-capture-status") || document.getElementById("face-capture-status");
    var errEl = root.querySelector("#face-capture-err") || document.getElementById("face-capture-err");
    var okEl = root.querySelector("#face-capture-success") || document.getElementById("face-capture-success");
    var btnStart = root.querySelector("#face-btn-start") || document.getElementById("face-btn-start");
    var btnReset = root.querySelector("#face-btn-reset") || document.getElementById("face-btn-reset");
    var btnSubmit = root.querySelector("#face-btn-submit") || document.getElementById("face-btn-submit");
    var btnCancel = root.querySelector("#face-btn-cancel") || document.getElementById("face-btn-cancel");
    var faceActionsEl = root.querySelector("#face-capture-actions") || document.getElementById("face-capture-actions");
    var faceSubmitLoaderEl =
      root.querySelector("#face-capture-submit-loader") || document.getElementById("face-capture-submit-loader");
    var submitWaitEl =
      root.querySelector("#face-capture-submit-wait") || document.getElementById("face-capture-submit-wait");
    var godmotherOptionsEl = document.getElementById("godmother-options");
    var godmotherStatusEl = document.getElementById("godmother-selection-status");
    var godmotherErrEl = document.getElementById("godmother-err");
    var godmotherOkEl = document.getElementById("godmother-success");
    var godmotherSubmitBtn = document.getElementById("godmother-submit");
    var faceCaptureCardEl = cardEl;
    var modelsReady = sharedModelsReady;
    var running = false;
    var stream = null;
    var stageIndex = 0;
    var stableCount = 0;
    var rafId = 0;
    var blobs = { neutral: null, smile: null, frown: null };
    var scores = [];
    var captureMetrics = {
      crop_source_side_px: null,
      face_height_ratio: null,
      camera_width: null,
      camera_height: null,
    };
    var godmotherOptions = [];
    var selectedGodmotherHandle = null;
    var captureAudioCtx = null;
    var submitInFlight = false;
    var rateLimitUntilMs = 0;
    var rateLimitTimer = null;

    function isRateLimited() {
      return Date.now() < rateLimitUntilMs;
    }

    function applyRateLimitCooldown(secs) {
      var waitMs = Math.max(1, Number(secs) || 60) * 1000;
      rateLimitUntilMs = Date.now() + waitMs;
      if (rateLimitTimer) window.clearInterval(rateLimitTimer);
      rateLimitTimer = window.setInterval(function () {
        if (!isRateLimited()) {
          window.clearInterval(rateLimitTimer);
          rateLimitTimer = null;
          syncCaptureActionButtons();
          return;
        }
        var remaining = Math.ceil((rateLimitUntilMs - Date.now()) / 1000);
        if (remaining > 0 && errEl && !errEl.hidden) {
          var msg =
            UI.rate_limited_retry && remaining
              ? UI.rate_limited_retry.replace("{seconds}", String(remaining))
              : UI.rate_limited || UI.err_submit || "";
          errEl.textContent = msg;
        }
      }, 1000);
    }

    function syncCaptureActionButtons(opts) {
      opts = opts || {};
      if (faceActionsEl) faceActionsEl.hidden = !!opts.hideActions;
      if (isAvatarRedraw) {
        var readyToSubmit = stageIndex >= CAPTURE_STEPS.length;
        if (btnStart) btnStart.hidden = readyToSubmit || !!opts.hideStart;
        if (btnReset) btnReset.hidden = !readyToSubmit;
        if (btnSubmit) {
          btnSubmit.hidden = !readyToSubmit;
          btnSubmit.disabled = !readyToSubmit || !!submitInFlight || isRateLimited();
        }
        return;
      }
      // Onboarding: auto-submit only — never show Submit.
      if (btnSubmit) {
        btnSubmit.hidden = true;
        btnSubmit.disabled = true;
      }
      if (btnStart) btnStart.hidden = stageIndex >= CAPTURE_STEPS.length || !!opts.hideStart;
      if (btnReset) btnReset.hidden = stageIndex < CAPTURE_STEPS.length;
    }

    function setFaceSubmitLoading(loading, waitText, waitHtml) {
      if (faceSubmitLoaderEl) faceSubmitLoaderEl.hidden = !loading;
      if (submitWaitEl) {
        if (waitHtml) {
          submitWaitEl.innerHTML = waitHtml;
        } else if (waitText) {
          submitWaitEl.textContent = waitText;
        }
      }
      if (loading) {
        syncCaptureActionButtons({ hideActions: true });
        if (btnCancel) btnCancel.hidden = true;
        return;
      }
      if (btnCancel) btnCancel.hidden = !isAvatarRedraw;
      syncCaptureActionButtons();
    }

    function detOptions() {
      return new faceapi.TinyFaceDetectorOptions({ inputSize: 416, scoreThreshold: 0.42 });
    }

    function faceCaptureErrorMessage(res, data) {
      if (res.status === 429) {
        var secs = data && data.retry_after_secs;
        if (secs) applyRateLimitCooldown(secs);
        if (secs && UI.rate_limited_retry) {
          return UI.rate_limited_retry.replace("{seconds}", String(secs));
        }
        return (data && data.error) || UI.rate_limited || UI.err_submit || res.statusText;
      }
      var code = data && data.code;
      var msg = "";
      if (code === "face_capture_rate_limited" && data && data.error) {
        return data.error;
      }
      if (code) {
        var artist = String(UI.avatar_artist_handle || "epic");
        var atArtist = artist.charAt(0) === "@" ? artist : "@" + artist;
        var byCode = {
          face_insufficient_diversity: UI.err_insufficient_diversity,
          face_vision_rejected: UI.err_vision_rejected,
          face_vision_rejected_not_live: UI.err_vision_rejected_not_live,
          face_vision_timeout: UI.err_vision_timeout,
          face_vision_rate_limited: UI.err_vision_rate_limited,
          face_capture_rate_limited: UI.rate_limited,
          face_vision_unavailable: UI.err_vision_failed,
          face_portrait_failed: UI.err_portrait_failed,
          face_quality_too_small: UI.err_quality_too_small,
          face_quality_low_resolution: UI.err_quality_low_resolution,
          face_quality_move_closer: UI.err_quality_move_closer,
          face_media_blocked: UI.err_media_blocked,
        };
        if (byCode[code]) {
          msg = byCode[code].replace(/\{artist_handle\}/g, atArtist);
        }
      }
      if (!msg && data && data.error) {
        msg = data.error;
      }
      if (!msg) {
        msg = UI.err_submit || res.statusText;
      }
      // Async pipeline stores the technical cause on `error` (status poll), not `detail`.
      if (
        data &&
        data.error &&
        data.code === "face_portrait_failed" &&
        msg.indexOf(String(data.error)) === -1
      ) {
        msg = msg + "\n\n" + String(data.error);
      } else if (data && data.detail) {
        msg = msg + "\n\n" + String(data.detail);
      }
      return msg;
    }

    function sleep(ms) {
      return new Promise(function (resolve) {
        window.setTimeout(resolve, ms);
      });
    }

    function restoreAfterCaptureSubmitError(msg) {
      submitInFlight = false;
      setFaceSubmitLoading(false);
      showErr(msg || UI.err_submit || "");
      if (isAvatarRedraw) {
        return;
      }
      setStatus(UI.submit_retry || UI.all_captured || "");
      syncCaptureActionButtons({ hideStart: false });
    }

    function showErr(msg) {
      if (!errEl) return;
      errEl.textContent = msg || "";
      errEl.hidden = !msg;
    }

    function showOk(msg) {
      if (!okEl) return;
      okEl.textContent = msg || "";
      okEl.hidden = !msg;
    }

    function showGodmotherErr(msg) {
      if (!godmotherErrEl) return;
      godmotherErrEl.textContent = msg || "";
      godmotherErrEl.hidden = !msg;
    }

    function showGodmotherOk(msg) {
      if (!godmotherOkEl) return;
      godmotherOkEl.textContent = msg || "";
      godmotherOkEl.hidden = !msg;
    }

    function setGodmotherStatus(msg) {
      if (godmotherStatusEl) godmotherStatusEl.textContent = msg || "";
    }

    function shakeNo(el) {
      if (!el) return;
      el.classList.remove("btn-shake-no");
      void el.offsetWidth;
      el.classList.add("btn-shake-no");
      window.setTimeout(function () {
        if (el) el.classList.remove("btn-shake-no");
      }, 420);
    }

    function escapeHtml(s) {
      return (s || "")
        .toString()
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
    }

    function avatarSrc(path) {
      var raw = (path || "").toString().trim();
      if (!raw) return "";
      if (/^https?:\/\//i.test(raw) || raw.charAt(0) === "/") return raw;
      return "/" + raw;
    }

    function displayCouncillorName(opt) {
      var base = (opt && (opt.label || opt.handle)) || "";
      var cleaned = base.toString().trim().replace(/\s+/g, " ");
      if (!cleaned) return "";
      return cleaned.split(" ")[0];
    }

    function refreshGodmotherSubmitState() {
      if (!godmotherSubmitBtn || isAvatarRedraw) return;
      godmotherSubmitBtn.disabled = !faceCaptureCompleted || !profileReady;
    }

    function renderGodmotherOptions() {
      if (!godmotherOptionsEl || isAvatarRedraw) return;
      if (!godmotherOptions.length) {
        godmotherOptionsEl.innerHTML = "";
        return;
      }
      godmotherOptionsEl.innerHTML = godmotherOptions
        .map(function (opt) {
          var selected = selectedGodmotherHandle === opt.handle ? " is-selected" : "";
          var title = escapeHtml(displayCouncillorName(opt));
          var community = escapeHtml(opt.community_label || "");
          var avatar = escapeHtml(avatarSrc(opt.avatar_path));
          return (
            '<button type="button" class="auth-text-btn godmother-option-btn' +
            selected +
            '" data-godmother-handle="' +
            escapeHtml(opt.handle) +
            '" role="listitem">' +
            '<span class="godmother-option-left">' +
            '<span class="guest-council-avatar-frame brand-frame brand-frame--toolbar godmother-option-avatar-frame">' +
            '<img class="guest-council-avatar-img godmother-option-avatar-img" src="' +
            avatar +
            '" alt="" width="64" height="64" loading="lazy" decoding="async">' +
            "</span>" +
            '<span class="godmother-option-title">' +
            title +
            "</span>" +
            "</span>" +
            '<span class="godmother-option-label">' +
            community +
            "</span>" +
            "</button>"
          );
        })
        .join("");
    }

    function bindGodmotherOptionClicks() {
      if (!godmotherOptionsEl || isAvatarRedraw) return;
      godmotherOptionsEl.addEventListener("click", function (e) {
        var target = e.target;
        if (!target) return;
        var btn = target.closest("[data-godmother-handle]");
        if (!btn) return;
        selectedGodmotherHandle = btn.getAttribute("data-godmother-handle");
        showGodmotherErr("");
        showGodmotherOk("");
        renderGodmotherOptions();
        refreshGodmotherSubmitState();
      });
    }

    async function loadGodmotherOptions() {
      if (!godmotherOptionsEl || isAvatarRedraw) return;
      if (!profileReady) {
        setGodmotherStatus(UI.profile_required_before_godmother || "");
        godmotherOptionsEl.innerHTML = "";
        refreshGodmotherSubmitState();
        return;
      }
      try {
        var res = await fetch("/me/godmother-options", {
          method: "GET",
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        });
        if (!res.ok) {
          showGodmotherErr(UI.godmother_load_failed || "");
          return;
        }
        var data = await res.json().catch(function () {
          return [];
        });
        if (!Array.isArray(data)) {
          showGodmotherErr(UI.godmother_load_failed || "");
          return;
        }
        if (!data.length) {
          showGodmotherErr(UI.godmother_empty || UI.godmother_load_failed || "");
          godmotherOptions = [];
          godmotherOptionsEl.innerHTML = "";
          refreshGodmotherSubmitState();
          return;
        }
        godmotherOptions = data;
        showGodmotherErr("");
        renderGodmotherOptions();
        refreshGodmotherSubmitState();
      } catch (_) {
        showGodmotherErr(UI.godmother_load_failed || "");
      }
    }

    async function submitGodmotherSelection() {
      showGodmotherErr("");
      showGodmotherOk("");
      if (!faceCaptureCompleted) {
        showGodmotherErr(UI.godmother_status_pending || "");
        return;
      }
      if (!profileReady) {
        showGodmotherErr(UI.profile_required_before_godmother || "");
        return;
      }
      if (!selectedGodmotherHandle) {
        shakeNo(godmotherSubmitBtn);
        showGodmotherErr(UI.godmother_select_required || "");
        return;
      }
      if (godmotherSubmitBtn) godmotherSubmitBtn.disabled = true;
      try {
        var res = await fetch("/me/godmother", {
          method: "POST",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify({ handle: selectedGodmotherHandle }),
        });
        var data = await res.json().catch(function () {
          return {};
        });
        if (res.ok) {
          showGodmotherOk(UI.godmother_status_selected || "");
          window.setTimeout(function () {
            window.location.href = "/home";
          }, 350);
          return;
        }
        if (res.status === 409) {
          showGodmotherOk(UI.godmother_conflict_selected || "");
          window.setTimeout(function () {
            window.location.href = "/home";
          }, 350);
          return;
        }
        showGodmotherErr(data.error || UI.err_submit || "");
      } catch (_) {
        showGodmotherErr(UI.network_error || "");
      } finally {
        refreshGodmotherSubmitState();
      }
    }

    function setStatus(msg) {
      if (statusEl) statusEl.textContent = msg || "";
    }

    function playCaptureBeep() {
      try {
        if (window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
          return;
        }
        var Ctx = window.AudioContext || window.webkitAudioContext;
        if (!Ctx) return;
        if (!captureAudioCtx) captureAudioCtx = new Ctx();
        var ctx = captureAudioCtx;
        function run() {
          try {
            var osc = ctx.createOscillator();
            var gain = ctx.createGain();
            osc.connect(gain);
            gain.connect(ctx.destination);
            osc.frequency.value = 880;
            osc.type = "sine";
            gain.gain.setValueAtTime(0.08, ctx.currentTime);
            gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.12);
            osc.start(ctx.currentTime);
            osc.stop(ctx.currentTime + 0.13);
          } catch (_) {
            /* ignore */
          }
        }
        if (ctx.state === "suspended") {
          ctx.resume().then(run).catch(run);
        } else {
          run();
        }
      } catch (_) {
        /* ignore */
      }
    }

    function yawFromLandmarks(lm) {
      var p = lm.positions;
      function avgRange(a, b) {
        var x = 0,
          y = 0,
          n = 0;
        for (var i = a; i < b; i++) {
          x += p[i].x;
          y += p[i].y;
          n++;
        }
        return { x: x / n, y: y / n };
      }
      var leftEye = avgRange(36, 42);
      var rightEye = avgRange(42, 48);
      var nose = p[30];
      var eyeMidX = (leftEye.x + rightEye.x) / 2;
      var iw = Math.hypot(rightEye.x - leftEye.x, rightEye.y - leftEye.y) || 1;
      return (nose.x - eyeMidX) / iw;
    }

    function yawOk(y) {
      return Math.abs(y) <= 0.18;
    }

    function stepInstruction(step) {
      if (!step) return "";
      if (step.promptKey && UI[step.promptKey]) return UI[step.promptKey];
      if (step.group === "neutral") return UI.stage_neutral || "";
      if (step.group === "smile") return UI.stage_smile || "";
      if (step.group === "frown") return UI.stage_frown || "";
      return "";
    }

    function formatStatus(hint, step) {
      var instr = stepInstruction(step);
      if (instr && hint) return instr + " · " + hint;
      return instr || hint || "";
    }

    function faceRegionLuminance(vid, crop) {
      if (!crop || !crop.ok || !workCanvas) return 255;
      var side = Math.max(8, Math.min(LIGHT_SAMPLE_PX, Math.floor(crop.sw), Math.floor(crop.sh)));
      workCanvas.width = side;
      workCanvas.height = side;
      var ctx = workCanvas.getContext("2d", { willReadFrequently: true });
      ctx.drawImage(vid, crop.sx, crop.sy, crop.sw, crop.sh, 0, 0, side, side);
      var data;
      try {
        data = ctx.getImageData(0, 0, side, side).data;
      } catch (_) {
        return 255;
      }
      var sum = 0;
      var n = 0;
      var i;
      for (i = 0; i < data.length; i += 4) {
        sum += 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
        n++;
      }
      return n ? sum / n : 255;
    }

    function landmarkHeadCrop(vid, landmarks, detection) {
      var p = landmarks.positions;
      var minX = Infinity;
      var maxX = -Infinity;
      var minY = Infinity;
      var maxY = -Infinity;
      var i;
      for (i = 0; i < p.length; i++) {
        minX = Math.min(minX, p[i].x);
        maxX = Math.max(maxX, p[i].x);
        minY = Math.min(minY, p[i].y);
        maxY = Math.max(maxY, p[i].y);
      }
      if (detection && detection.box) {
        var b = detection.box;
        minX = Math.min(minX, b.x);
        maxX = Math.max(maxX, b.x + b.width);
        minY = Math.min(minY, b.y);
        maxY = Math.max(maxY, b.y + b.height);
      }
      var vw = vid.videoWidth;
      var vh = vid.videoHeight;
      if (!vw || !vh) return { ok: false };
      var faceW = maxX - minX;
      var faceH = maxY - minY;
      if (faceH < 8 || faceW < 8) return { ok: false };
      var topPad = faceH * HEAD_TOP_PAD_RATIO;
      var bottomPad = faceH * (HEAD_BOTTOM_PAD_RATIO + BEARD_EXTRA_BOTTOM_PAD_RATIO);
      var sidePad = faceW * HEAD_SIDE_PAD_RATIO;
      var cx = (minX + maxX) / 2;
      var cy = (minY + maxY) / 2 + (bottomPad - topPad) * 0.15;
      var sw = faceW + 2 * sidePad;
      var sh = faceH + topPad + bottomPad;
      var side = Math.max(sw, sh);
      var sx = cx - side / 2;
      var sy = cy - side / 2;
      if (sx < 0) sx = 0;
      if (sy < 0) sy = 0;
      if (sx + side > vw) sx = Math.max(0, vw - side);
      if (sy + side > vh) sy = Math.max(0, vh - side);
      var cropW = Math.min(side, vw - sx);
      var cropH = Math.min(side, vh - sy);
      return {
        ok: true,
        sx: sx,
        sy: sy,
        sw: cropW,
        sh: cropH,
        cropSourceSide: Math.min(cropW, cropH),
        faceHeightRatio: faceH / vh,
      };
    }

    function cropHeadTo512(vid, landmarks, detection) {
      var crop = landmarkHeadCrop(vid, landmarks, detection);
      if (!crop.ok) return crop;
      if (crop.faceHeightRatio < MIN_FACE_HEIGHT_RATIO || crop.cropSourceSide < MIN_CROP_SOURCE_SIDE) {
        return { ok: false, tooFar: true, faceHeightRatio: crop.faceHeightRatio };
      }
      var ctx = workCanvas.getContext("2d");
      workCanvas.width = TARGET;
      workCanvas.height = TARGET;
      ctx.drawImage(vid, crop.sx, crop.sy, crop.sw, crop.sh, 0, 0, TARGET, TARGET);
      return {
        ok: true,
        cropSourceSide: crop.cropSourceSide,
        faceHeightRatio: crop.faceHeightRatio,
      };
    }

    function stopCamera() {
      running = false;
      if (rafId) cancelAnimationFrame(rafId);
      rafId = 0;
      if (stream) {
        stream.getTracks().forEach(function (t) {
          t.stop();
        });
        stream = null;
      }
      if (videoEl) videoEl.srcObject = null;
    }

    function resetCapture() {
      stopCamera();
      stageIndex = 0;
      stableCount = 0;
      blobs = { neutral: null, smile: null, frown: null };
      scores = [];
      captureMetrics = {
        crop_source_side_px: null,
        face_height_ratio: null,
        camera_width: null,
        camera_height: null,
      };
      if (!isAvatarRedraw) {
        faceCaptureCompleted = false;
        showGodmotherErr("");
        showGodmotherOk("");
        setGodmotherStatus(UI.godmother_status_pending || "");
      }
      showErr("");
      showOk("");
      setStatus("");
      syncCaptureActionButtons();
      refreshGodmotherSubmitState();
    }

    function captureForStep(step, nextIndex) {
      return new Promise(function (resolve) {
        workCanvas.toBlob(
          function (blob) {
            blobs[step.group] = blob;
            playCaptureBeep();
            stageIndex = nextIndex;
            stableCount = 0;
            if (stageIndex >= CAPTURE_STEPS.length) {
              running = false;
              if (rafId) cancelAnimationFrame(rafId);
              rafId = 0;
              stopCamera();
              if (btnStart) btnStart.hidden = true;
              if (isAvatarRedraw) {
                // Paid redraw charges Noms, so keep an explicit confirm button.
                setStatus("");
                if (btnSubmit) {
                  btnSubmit.hidden = false;
                  btnSubmit.disabled = false;
                }
                if (btnReset) btnReset.hidden = false;
              } else {
                // Onboarding automation: once a face has been captured for all three expressions,
                // submit automatically. The server confirms a face (vision) and advances; if no
                // face is detected the user is returned to the capture step.
                setStatus(UI.all_captured || UI.pipeline_stage_uploading || "");
                submitCapture();
              }
              resolve();
            } else {
              setStatus(UI.capture_pause || "Get ready for the next photo…");
              sleep(CAPTURE_PAUSE_MS).then(function () {
                setStatus(stepInstruction(CAPTURE_STEPS[stageIndex]));
                resolve();
              });
            }
          },
          "image/jpeg",
          0.88
        );
      });
    }

    async function loop() {
      if (!running || stageIndex >= CAPTURE_STEPS.length) return;
      var step = CAPTURE_STEPS[stageIndex];
      try {
        var r = await faceapi
          .detectSingleFace(videoEl, detOptions())
          .withFaceLandmarks();
        if (!r) {
          stableCount = 0;
          setStatus(formatStatus(UI.err_no_face || "", step));
          rafId = requestAnimationFrame(loop);
          return;
        }
        var yaw = yawFromLandmarks(r.landmarks);
        if (!yawOk(yaw)) {
          stableCount = 0;
          setStatus(formatStatus(UI.face_forward_hint || "Keep facing the screen.", step));
          rafId = requestAnimationFrame(loop);
          return;
        }
        var cropPreview = landmarkHeadCrop(videoEl, r.landmarks, r.detection);
        if (!cropPreview.ok || cropPreview.faceHeightRatio < MIN_FACE_HEIGHT_RATIO) {
          stableCount = 0;
          setStatus(
            formatStatus(UI.err_move_closer || UI.frame_head_hint || "", step)
          );
          rafId = requestAnimationFrame(loop);
          return;
        }
        var luminance = faceRegionLuminance(videoEl, cropPreview);
        if (luminance < MIN_FACE_LUMINANCE) {
          stableCount = 0;
          setStatus(formatStatus(UI.err_dark || UI.frame_head_hint || "", step));
          rafId = requestAnimationFrame(loop);
          return;
        }
        stableCount++;
        if (stableCount < STABLE_FRAMES) {
          var steadyBase = UI.steady || "Hold still";
          setStatus(
            formatStatus(steadyBase + " (" + stableCount + "/" + STABLE_FRAMES + ")", step)
          );
          rafId = requestAnimationFrame(loop);
          return;
        }
        var cropped = cropHeadTo512(videoEl, r.landmarks, r.detection);
        if (!cropped.ok) {
          stableCount = 0;
          setStatus(
            formatStatus(UI.err_move_closer || UI.frame_head_hint || "", step)
          );
          rafId = requestAnimationFrame(loop);
          return;
        }
        captureMetrics.crop_source_side_px = cropped.cropSourceSide;
        captureMetrics.face_height_ratio = cropped.faceHeightRatio;
        captureMetrics.camera_width = videoEl.videoWidth || null;
        captureMetrics.camera_height = videoEl.videoHeight || null;
        scores.push(r.detection.score);
        await captureForStep(step, stageIndex + 1);
        if (stageIndex < CAPTURE_STEPS.length) rafId = requestAnimationFrame(loop);
      } catch (e) {
        showErr(UI.err_detection || String(e));
        resetCapture();
      }
    }

    async function startCamera() {
      showErr("");
      showOk("");
      if (!modelsReady) {
        if (btnStart) btnStart.disabled = true;
        setStatus(UI.models_ready || "Loading face detection…");
        try {
          await preloadFaceCaptureModels();
          modelsReady = sharedModelsReady;
          setStatus(UI.models_ready || "");
        } catch (e) {
          showErr(UI.err_models || String(e.message || e));
          if (btnStart) btnStart.disabled = false;
          return;
        }
        if (btnStart) btnStart.disabled = false;
      }
      if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
        showErr(UI.err_camera || "");
        return;
      }
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: {
            facingMode: "user",
            width: { ideal: 1280, min: 720 },
            height: { ideal: 720, min: 480 },
          },
          audio: false,
        });
        videoEl.srcObject = stream;
        await videoEl.play();
        try {
          var AC = window.AudioContext || window.webkitAudioContext;
          if (AC) {
            if (!captureAudioCtx) captureAudioCtx = new AC();
            await captureAudioCtx.resume();
          }
        } catch (_) {
          /* ignore */
        }
      } catch (e) {
        showErr(UI.err_camera || String(e));
        return;
      }
      stageIndex = 0;
      stableCount = 0;
      blobs = { neutral: null, smile: null, frown: null };
      scores = [];
      captureMetrics = {
        crop_source_side_px: null,
        face_height_ratio: null,
        camera_width: null,
        camera_height: null,
      };
      running = true;
      syncCaptureActionButtons();
      setStatus(
        formatStatus(UI.frame_head_hint || "", CAPTURE_STEPS[0])
      );
      rafId = requestAnimationFrame(loop);
    }

    function avatarGeneratingWaitHtml() {
      var handle = String(UI.avatar_artist_handle || "epic")
        .trim()
        .replace(/^@+/, "");
      var atHandle = "@" + handle;
      var url = String(UI.avatar_artist_profile_url || "/public/" + encodeURIComponent(handle));
      var suffix =
        UI.avatar_redraw_generating_suffix || "is painting your new portrait…";
      var aria = UI.avatar_artist_link_aria || "Open " + atHandle + " public profile";
      return (
        '<a class="profile-handle-link face-capture-artist-link" href="' +
        escapeHtml(url) +
        '" aria-label="' +
        escapeHtml(aria) +
        '">' +
        escapeHtml(atHandle) +
        "</a> " +
        escapeHtml(suffix)
      );
    }

    function artistHandleAt() {
      var handle = String(UI.avatar_artist_handle || "epic")
        .trim()
        .replace(/^@+/, "");
      return "@" + handle;
    }

    function pipelineStageMessage(data) {
      var atHandle = artistHandleAt();
      var stage =
        (data && data.stage) ||
        (data && data.status === "queued" ? "queued" : null) ||
        (data && data.status === "processing" ? "painting" : null);
      var template = {
        uploading: UI.pipeline_stage_uploading,
        queued: UI.pipeline_stage_queued,
        validating: UI.pipeline_stage_validating,
        painting: UI.pipeline_stage_painting,
      }[stage];
      if (!template) return null;
      return String(template).replace(/\{artist_handle\}/g, atHandle);
    }

    function setPipelineWaitMessage(data, fallbackHtml) {
      var text = pipelineStageMessage(data);
      if (text) {
        setFaceSubmitLoading(true, text, null);
        setStatus(text);
        return;
      }
      if (fallbackHtml) {
        setFaceSubmitLoading(true, null, fallbackHtml);
      }
    }

    async function pollFaceCaptureStatus(pendingId) {
      var maxAttempts = 180;
      for (var attempt = 0; attempt < maxAttempts; attempt++) {
        if (attempt > 0) {
          await sleep(1000);
        }
        var res = await fetch(
          "/me/face-capture/status/" + encodeURIComponent(String(pendingId)),
          {
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          }
        );
        var data = await res.json().catch(function () {
          return {};
        });
        if (!res.ok) {
          throw new Error(
            (data && data.error) || UI.err_submit || res.statusText || "status failed"
          );
        }
        setPipelineWaitMessage(data, avatarGeneratingWaitHtml());
        if (data.status === "complete") {
          return data;
        }
        if (data.status === "failed") {
          var errRes = { status: 400 };
          throw new Error(faceCaptureErrorMessage(errRes, data));
        }
      }
      throw new Error(UI.pipeline_timeout || UI.err_submit || "Portrait generation timed out.");
    }

    function buildCaptureFormData() {
      var q = scores.length === 3 ? (scores[0] + scores[1] + scores[2]) / 3 : null;
      var fd = new FormData();
      fd.append(
        "meta",
        JSON.stringify({
          quality_score: q,
          camera_width: captureMetrics.camera_width,
          camera_height: captureMetrics.camera_height,
          crop_source_side_px: captureMetrics.crop_source_side_px,
          face_height_ratio: captureMetrics.face_height_ratio,
        })
      );
      fd.append("neutral", blobs.neutral, "neutral.jpg");
      fd.append("smile", blobs.smile, "smile.jpg");
      fd.append("frown", blobs.frown, "frown.jpg");
      return fd;
    }

    // Avatar redraw re-captures fresh photos and posts them to the paid redraw endpoint.
    async function submitAvatarRedraw() {
      var fd = buildCaptureFormData();
      setPipelineWaitMessage({ stage: "uploading" }, avatarGeneratingWaitHtml());
      try {
        var res = await fetch("/me/avatar-redraw", {
          method: "POST",
          body: fd,
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        });
        var data = await res.json().catch(function () {
          return {};
        });
        if (!res.ok || res.status !== 202 || !data.pending_id) {
          var errMsg = faceCaptureErrorMessage(res, data);
          showErr(errMsg);
          submitInFlight = false;
          setFaceSubmitLoading(false);
          syncCaptureActionButtons();
          return;
        }
        var result = await pollFaceCaptureStatus(data.pending_id);
        stopCamera();
        showOk(UI.avatar_redraw_complete || UI.success || "");
        var detail = {
          avatar_path: result && result.avatar_path ? String(result.avatar_path) : "",
          noms_spent: result && result.noms_spent,
          noms_balance: result && result.noms_balance,
        };
        if (options.suppressInlineSuccess) showOk("");
        if (typeof options.onComplete === "function") {
          options.onComplete(detail);
        } else {
          document.dispatchEvent(new CustomEvent("app:avatar-redraw-complete", { detail: detail }));
        }
      } catch (e) {
        showErr(String(e.message || e));
        submitInFlight = false;
        setFaceSubmitLoading(false);
      }
    }

    async function submitCapture() {
      if (submitInFlight || isRateLimited()) return;
      showErr("");
      showOk("");
      if (!blobs.neutral || !blobs.smile || !blobs.frown) {
        showErr(UI.err_incomplete || "");
        return;
      }
      if (isAvatarRedraw) {
        submitInFlight = true;
        await submitAvatarRedraw();
        submitInFlight = false;
        return;
      }
      var fd = buildCaptureFormData();
      submitInFlight = true;
      setPipelineWaitMessage({ stage: "uploading" }, avatarGeneratingWaitHtml());
      try {
        var res = await fetch("/me/face-capture", {
          method: "POST",
          body: fd,
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        });
        var data = await res.json().catch(function () {
          return {};
        });
        if (!res.ok || res.status !== 202 || !data.pending_id) {
          restoreAfterCaptureSubmitError(faceCaptureErrorMessage(res, data));
          return;
        }
        await pollFaceCaptureStatus(data.pending_id);
        submitInFlight = false;
        faceCaptureCompleted = true;
        showOk(UI.success || "");
        var artistEmbed = document.querySelector(".face-capture-artist-embed");
        if (artistEmbed) artistEmbed.hidden = true;
        if (profileReady) {
          setFaceSubmitLoading(false);
          if (faceCaptureCardEl) faceCaptureCardEl.hidden = true;
          if (godmotherCardEl) godmotherCardEl.hidden = false;
          setGodmotherStatus(UI.godmother_status_ready || "");
          loadGodmotherOptions();
          refreshGodmotherSubmitState();
        } else {
          setGodmotherStatus(UI.profile_required_before_godmother || "");
          window.location.href = "/home?onboarding=profile";
        }
      } catch (e) {
        restoreAfterCaptureSubmitError(String(e.message || e) || UI.network_error || String(e));
      }
    }

    async function loadModels() {
      if (btnStart) btnStart.disabled = true;
      try {
        await preloadFaceCaptureModels();
        modelsReady = sharedModelsReady;
        setStatus(UI.models_ready || "");
        showErr("");
      } catch (e) {
        showErr(UI.err_models || String(e.message || e));
      } finally {
        if (btnStart && !running && stageIndex < CAPTURE_STEPS.length) {
          btnStart.disabled = false;
        }
      }
    }

    function handleCancel() {
      stopCamera();
      if (typeof options.onCancel === "function") {
        options.onCancel();
      } else if (isAvatarRedraw) {
        document.dispatchEvent(new CustomEvent("app:avatar-redraw-cancel"));
      }
    }

    if (faceCaptureCardEl && faceCaptureCompleted && profileReady && !isAvatarRedraw) {
      faceCaptureCardEl.hidden = true;
    }
    if (btnCancel && isAvatarRedraw) btnCancel.hidden = false;
    else if (btnCancel) btnCancel.hidden = true;

    if (!godmotherOnly) {
      loadModels();
    }
    if (!isAvatarRedraw) {
      bindGodmotherOptionClicks();
      loadGodmotherOptions();
      setGodmotherStatus(
        !profileReady
          ? UI.profile_required_before_godmother || ""
          : faceCaptureCompleted
          ? UI.godmother_status_ready || ""
          : UI.godmother_status_pending || ""
      );
    }
    if (!godmotherOnly) {
      if (btnStart) btnStart.addEventListener("click", startCamera);
      if (btnReset) btnReset.addEventListener("click", resetCapture);
      if (btnSubmit) btnSubmit.addEventListener("click", submitCapture);
    }
    if (btnCancel) btnCancel.addEventListener("click", handleCancel);
    if (godmotherSubmitBtn && !isAvatarRedraw) {
      godmotherSubmitBtn.addEventListener("click", submitGodmotherSelection);
      refreshGodmotherSubmitState();
    }

    return {
      stop: stopCamera,
      reset: resetCapture,
    };
  }

  window.AppFaceCapture = {
    bootstrap: bootstrapFaceCapture,
    preloadModels: preloadFaceCaptureModels,
  };

  function autoInitOnboardingPage() {
    if (!document.querySelector(".face-capture-main") && !document.querySelector(".onboarding-godmother-main")) {
      return;
    }
    bootstrapFaceCapture({ root: document });
  }

  if (document.readyState === "loading") {
    window.addEventListener("load", autoInitOnboardingPage);
  } else {
    autoInitOnboardingPage();
  }
})();

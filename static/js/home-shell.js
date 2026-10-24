/**
 * Signed-in home onboarding: terms first, then profile form; country may be prefilled from geolocation + reverse-geocode.
 */
(function () {
  "use strict";

  var CARD_URL = "/home/profile-completion-card";
  var CONSENT_CARD_URL = "/home/terms-consent-card";
  var CREATE_POST_CARD_URL = "/home/create-post-card";
  var SEARCH_ADD_CARD_URL = "/home/search-add-card";
  var CREATE_ENTITY_CARD_URL = "/home/create-entity-card";
  var FEED_DAY_URL = "/home/feed-day";
  var SEARCH_ENTITIES_URL = "/social-entities/search";
  var EFFECTIVE_CONSTITUTION_URL = "/social-entities/effective-constitution";
  var PRIVATE_PROFILE_CARD_BASE = "/home/private-profile-card/";
  var FACE_CAPTURE_CARD_URL = "/home/face-capture-card";

  function shakeCtaButton(el) {
    if (!el) return;
    el.classList.remove("btn-shake-no");
    void el.offsetWidth;
    el.classList.add("btn-shake-no");
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function mutationHeaders(extra) {
    var h = { Accept: "application/json" };
    var meta = document.querySelector('meta[name="csrf-token"]');
    var token = meta ? String(meta.getAttribute("content") || "").trim() : "";
    if (token) h["X-CSRF-Token"] = token;
    if (extra) {
      for (var k in extra) {
        if (Object.prototype.hasOwnProperty.call(extra, k)) h[k] = extra[k];
      }
    }
    return h;
  }

  function debounce(fn, waitMs) {
    var t = null;
    return function () {
      var args = arguments;
      if (t) clearTimeout(t);
      t = setTimeout(function () {
        fn.apply(null, args);
      }, waitMs);
    };
  }

  var HandleInput = window.AppHandleInput || {};
  var EntityFormCommon = window.CreateEntityFormCommon || {};
  var normalizeHandleCandidate = HandleInput.normalizeHandleCandidate || function (raw) {
    var s = String(raw || "").trim().toLowerCase();
    s = s.replace(/^@+/, "");
    return s;
  };
  var handleHasMinLength = HandleInput.handleHasMinLength || function (raw) {
    return normalizeHandleCandidate(raw).length >= 3;
  };
  var applyHandleInputDefaults = HandleInput.applyHandleInputDefaults || function () {};

  function ensureHandleAvailabilityBadge(inputEl, variant) {
    if (!inputEl || !inputEl.parentNode) return null;
    var parent = inputEl.parentNode;
    var forInput = String(inputEl.id || "").trim();
    var existing = forInput
      ? parent.querySelector('.handle-availability-badge[data-for-input="' + forInput + '"]')
      : null;
    if (existing) return existing;
    var badge = document.createElement("span");
    badge.className = "handle-availability-badge handle-availability-badge--" + (variant || "inline");
    badge.setAttribute("aria-live", "polite");
    if (forInput) badge.setAttribute("data-for-input", forInput);
    badge.hidden = true;
    var labelEl = parent.querySelector('label[for="' + inputEl.id + '"]');
    if (labelEl && labelEl.parentNode === parent) {
      labelEl.appendChild(badge);
    } else {
      inputEl.insertAdjacentElement("beforebegin", badge);
    }
    return badge;
  }

  function setHandleAvailabilityBadge(badgeEl, msg, mode) {
    if (!badgeEl) return;
    badgeEl.classList.remove(
      "handle-availability-badge--checking",
      "handle-availability-badge--error",
      "handle-availability-badge--success"
    );
    if (!msg) {
      badgeEl.hidden = true;
      badgeEl.textContent = "";
      return;
    }
    badgeEl.hidden = false;
    badgeEl.textContent = msg;
    if (mode === "checking") {
      badgeEl.classList.add("handle-availability-badge--checking");
    } else if (mode === "success") {
      badgeEl.classList.add("handle-availability-badge--success");
    } else {
      badgeEl.classList.add("handle-availability-badge--error");
    }
  }

  /** Latest birth date (local calendar) so the person is at least `years` old today. */
  function isoDateMinusYears(years) {
    var d = new Date();
    d.setFullYear(d.getFullYear() - years);
    var y = d.getFullYear();
    var m = String(d.getMonth() + 1).padStart(2, "0");
    var day = String(d.getDate()).padStart(2, "0");
    return y + "-" + m + "-" + day;
  }

  function resolveCountryFromGeolocation() {
    return new Promise(function (resolve) {
      if (!navigator.geolocation) {
        resolve(null);
        return;
      }
      navigator.geolocation.getCurrentPosition(
        function (pos) {
          fetch("/me/reverse-geocode", {
            method: "POST",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify({
              latitude: pos.coords.latitude,
              longitude: pos.coords.longitude,
            }),
            credentials: "same-origin",
          })
            .then(function (r) {
              if (r.status === 429 || !r.ok) return null;
              return r.json();
            })
            .then(function (data) {
              if (data && data.country_code && typeof data.country_code === "string") {
                resolve(data.country_code.trim().toUpperCase());
              } else {
                resolve(null);
              }
            })
            .catch(function () {
              resolve(null);
            });
        },
        function () {
          resolve(null);
        },
        { enableHighAccuracy: false, maximumAge: 300000, timeout: 120000 }
      );
    });
  }

  function applyDetectedCountry(detectedCc, lock) {
    var sel = document.getElementById("profile-country");
    if (!sel) return;
    var note = document.getElementById("profile-country-detected-note");
    var hintDefault = document.getElementById("profile-country-hint-default");
    if (!detectedCc) {
      sel.disabled = false;
      sel.required = true;
      if (note) note.hidden = true;
      if (hintDefault) hintDefault.hidden = false;
      return;
    }
    var opt = sel.querySelector('option[value="' + detectedCc + '"]');
    if (!opt) {
      sel.disabled = false;
      sel.required = true;
      if (note) note.hidden = true;
      if (hintDefault) hintDefault.hidden = false;
      return;
    }
    sel.value = detectedCc;
    if (lock) {
      sel.disabled = true;
      sel.required = false;
      if (note) note.hidden = false;
      if (hintDefault) hintDefault.hidden = true;
    } else {
      sel.disabled = false;
      sel.required = true;
      if (note) note.hidden = true;
      if (hintDefault) hintDefault.hidden = false;
    }
  }

  /** After profile card mount: resolve country from browser location and lock the select when matched. */
  function startProfileCountryDetection() {
    var applied = false;
    function tryApply(cc) {
      if (!cc || applied) return;
      if (!document.getElementById("profile-country")) return;
      applyDetectedCountry(cc, true);
      applied = true;
    }
    resolveCountryFromGeolocation().then(tryApply);
    if (navigator.permissions && typeof navigator.permissions.query === "function") {
      navigator.permissions
        .query({ name: "geolocation" })
        .then(function (result) {
          if (result.state === "granted") {
            resolveCountryFromGeolocation().then(tryApply);
          }
          result.addEventListener("change", function () {
            if (result.state === "granted") {
              resolveCountryFromGeolocation().then(tryApply);
            }
          });
        })
        .catch(function () {});
    }
  }

  function bindTermsConsentForm(UI) {
    var form = document.getElementById("terms-consent-form");
    var msg = document.getElementById("terms-consent-msg");
    var btn = document.getElementById("terms-consent-submit");
    if (!form || !msg || !btn) return;
    if (form.getAttribute("data-terms-consent-bound") === "1") return;
    form.setAttribute("data-terms-consent-bound", "1");

    form.addEventListener("submit", async function (e) {
      e.preventDefault();
      msg.textContent = "";
      var adult = document.getElementById("terms-adult");
      var constitution = document.getElementById("terms-constitution");
      if (!adult || !adult.checked) {
        msg.textContent = UI.must_confirm_adult || "Please confirm you are 16 or older.";
        return;
      }
      if (!constitution || !constitution.checked) {
        msg.textContent =
          UI.must_confirm_constitution || "Please confirm you agree to the Root Constitution.";
        return;
      }
      btn.disabled = true;
      try {
        var r = await fetch("/me/terms-consent", {
          method: "POST",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify({ adult_ack: true, constitution_ack: true }),
          credentials: "same-origin",
        });
        var data = await r.json().catch(function () {
          return {};
        });
        if (r.ok && data.next) {
          window.location.href = data.next;
          return;
        }
        msg.textContent = data.error || UI.terms_consent_failed || "Could not save your consent.";
      } catch (_) {
        msg.textContent = UI.network_error_detail || "Network error.";
      } finally {
        btn.disabled = false;
      }
    });
  }

  function bindProfileForm(UI) {
    var form = document.getElementById("profile-completion-form");
    var msg = document.getElementById("profile-completion-msg");
    var btn = document.getElementById("profile-completion-next");
    if (!form || !msg || !btn) return;

    if (form.getAttribute("data-profile-step1-bound") === "1") return;
    form.setAttribute("data-profile-step1-bound", "1");
    var profileHintMap = {
      "profile-handle": form.querySelector('.hint[data-hint-for="profile-handle"]'),
      "profile-birth-date": form.querySelector('.hint[data-hint-for="profile-birth-date"]'),
      "profile-country": form.querySelector('.hint[data-hint-for="profile-country"]'),
    };
    var profileHandleHintDefault =
      (profileHintMap["profile-handle"] && profileHintMap["profile-handle"].textContent) || "";
    var profileHandleState = { status: "unknown", token: 0 };
    var profileHandleInput = document.getElementById("profile-handle");
    var profileHandleBadge = ensureHandleAvailabilityBadge(profileHandleInput, "label");

    function clearProfileHintErrors() {
      Object.keys(profileHintMap).forEach(function (id) {
        var hintEl = profileHintMap[id];
        if (hintEl) hintEl.classList.remove("hint--error");
      });
    }

    function hasValidProfileInputs() {
      clearProfileHintErrors();
      var invalid = false;
      Object.keys(profileHintMap).forEach(function (id) {
        var field = document.getElementById(id);
        var hintEl = profileHintMap[id];
        if (!field || !hintEl) return;
        if (id === "profile-country" && field.disabled) {
          if (!field.value.trim()) {
            hintEl.classList.add("hint--error");
            invalid = true;
          }
          return;
        }
        if (!field.checkValidity()) {
          hintEl.classList.add("hint--error");
          invalid = true;
        }
      });
      if (profileHandleState.status !== "available") {
        var handleHintEl = profileHintMap["profile-handle"];
        if (handleHintEl) handleHintEl.classList.add("hint--error");
        invalid = true;
      }
      return !invalid;
    }

    function setProfileHandleHint(msg, mode) {
      var hintEl = profileHintMap["profile-handle"];
      if (hintEl) {
        hintEl.textContent = msg || profileHandleHintDefault;
        hintEl.classList.remove(
          "hint--error",
          "create-post-submit-status--error",
          "create-post-submit-status--success",
          "handle-availability--taken",
          "handle-availability--ready"
        );
        if (mode === "error") {
          hintEl.classList.add("hint--error", "create-post-submit-status--error", "handle-availability--taken");
        } else if (mode === "success") {
          hintEl.classList.add("create-post-submit-status--success", "handle-availability--ready");
        }
      }
      if (mode === "error") {
        setHandleAvailabilityBadge(profileHandleBadge, msg, "error");
      } else if (mode === "success") {
        setHandleAvailabilityBadge(profileHandleBadge, msg, "success");
      } else if (profileHandleState.status === "checking") {
        setHandleAvailabilityBadge(profileHandleBadge, msg, "checking");
      } else {
        setHandleAvailabilityBadge(profileHandleBadge, "", "");
      }
    }

    var checkProfileHandleDebounced = debounce(function () {
      var handleField = document.getElementById("profile-handle");
      if (!handleField) return;
      var normalized = normalizeHandleCandidate(handleField.value);
      if (!handleHasMinLength(normalized)) {
        profileHandleState.status = "too_short";
        setProfileHandleHint(UI.handle_min_chars || profileHandleHintDefault, "error");
        return;
      }
      profileHandleState.status = "checking";
      setProfileHandleHint(UI.handle_checking || "Checking handle...", null);
      var token = profileHandleState.token + 1;
      profileHandleState.token = token;
      fetch("/social-entities/handle-availability?handle=" + encodeURIComponent(normalized), {
        credentials: "same-origin",
        headers: { Accept: "application/json" },
      })
        .then(function (r) {
          if (!r.ok) throw new Error("handle_check_failed");
          return r.json();
        })
        .then(function (data) {
          if (profileHandleState.token !== token) return;
          var status = String((data && data.status) || "");
          profileHandleState.status = status;
          if (status === "available") {
            setProfileHandleHint(UI.handle_ready || "Ready to Go", "success");
            return;
          }
          if (status === "taken") {
            setProfileHandleHint(UI.handle_taken || "Already taken", "error");
            return;
          }
          if (status === "reserved") {
            setProfileHandleHint(UI.handle_reserved || "This handle is reserved", "error");
            return;
          }
          if (status === "invalid") {
            setProfileHandleHint(UI.handle_invalid || profileHandleHintDefault, "error");
            return;
          }
          if (status === "too_short") {
            setProfileHandleHint(UI.handle_min_chars || profileHandleHintDefault, "error");
            return;
          }
          setProfileHandleHint(profileHandleHintDefault, null);
        })
        .catch(function () {
          if (profileHandleState.token !== token) return;
          profileHandleState.status = "error";
          setProfileHandleHint(UI.handle_check_failed || UI.request_failed || "Handle check failed.", "error");
        });
    }, 220);

    var birthEl = document.getElementById("profile-birth-date");
    var minAgeYears =
      UI && typeof UI.min_age_years === "number" ? UI.min_age_years : 16;
    if (birthEl) birthEl.max = isoDateMinusYears(minAgeYears);

    async function submitProfileStep() {
      msg.textContent = "";
      if (birthEl) birthEl.max = isoDateMinusYears(minAgeYears);
      if (!hasValidProfileInputs()) {
        shakeCtaButton(btn);
        return;
      }
      btn.disabled = true;
      var fd = new FormData(form);
      var countrySel = document.getElementById("profile-country");
      var country = countrySel
        ? countrySel.value.trim().toUpperCase()
        : (fd.get("country_code") || "").toString().trim().toUpperCase();
      var handleRaw = normalizeHandleCandidate((fd.get("handle") || "").toString());
      var draft = {
        handle: handleRaw,
        given_name: (fd.get("given_name") || "").toString().trim() || null,
        family_name: (fd.get("family_name") || "").toString().trim() || null,
        birth_date: (fd.get("birth_date") || "").toString().trim(),
        country_code: country,
      };
      if (draft.given_name === "") draft.given_name = null;
      if (draft.family_name === "") draft.family_name = null;

      try {
        var r = await fetch("/me/human-profile", {
          method: "POST",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify(draft),
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          msg.textContent = d429.error || UI.card_rate_limited;
          return;
        }
        if (r.ok) {
          window.location.href = "/onboarding";
          return;
        }
        var d = await r.json().catch(function () {
          return {};
        });
        msg.textContent = d.error || UI.request_failed;
      } catch (_) {
        msg.textContent = UI.network_error_detail;
      } finally {
        btn.disabled = false;
      }
    }

    form.addEventListener("submit", function (e) {
      e.preventDefault();
      submitProfileStep();
    });
    btn.addEventListener("click", function () {
      submitProfileStep();
    });
    form.addEventListener("input", function () {
      clearProfileHintErrors();
    });
    if (profileHandleInput) {
      applyHandleInputDefaults(profileHandleInput, { keepPrefixOnFocus: true, useDefaultPlaceholder: true });
      profileHandleInput.addEventListener("input", checkProfileHandleDebounced);
      checkProfileHandleDebounced();
    }
  }

  function run() {
    var shell = document.querySelector(".app-shell");
    if (!shell) return;

    var needsProfile = shell.getAttribute("data-needs-profile") === "1";
    var needsConsent = shell.getAttribute("data-needs-consent") === "1";

    // Server onboarding_gate routes consent/profile steps on /home; only send to /onboarding for face capture.
    var path = (window.location && window.location.pathname) || "";
    var onHomeRoot = path === "/home" || path === "/home/";
    if (
      onHomeRoot &&
      !needsConsent &&
      !needsProfile &&
      shell.getAttribute("data-face-capture-ready") !== "1"
    ) {
      window.location.replace("/onboarding");
      return;
    }

    var urlParams = new URLSearchParams(window.location.search);

    var i18nEl = document.getElementById("home-shell-i18n");
    var profilePanel = document.getElementById("app-home-profile-panel");
    var createPanel = document.getElementById("app-home-create-panel");
    var feedPanel = document.getElementById("app-home-feed-panel");
    var appMain = document.getElementById("app-main");
    var loadingEl = document.getElementById("app-home-profile-loading");
    var loadingMsg = document.getElementById("app-home-profile-loading-msg");
    var mountEl = document.getElementById("app-home-profile-mount");
    var createLoadingEl = document.getElementById("app-home-create-loading");
    var createLoadingMsg = document.getElementById("app-home-create-loading-msg");
    var createMountEl = document.getElementById("app-home-create-mount");
    if (!i18nEl || !profilePanel || !loadingEl || !mountEl) return;

    var UI;
    try {
      UI = JSON.parse(i18nEl.textContent);
    } catch (parseErr) {
      if (feedPanel) {
        feedPanel.hidden = false;
        feedPanel.innerHTML =
          '<section class="card app-card"><div class="card-accent" role="presentation"></div><p class="sub" role="alert">Home shell failed to start (bad i18n JSON). Hard-refresh the page.</p></section>';
      }
      console.error("home-shell i18n JSON.parse failed", parseErr);
      return;
    }
    var onboardingMode = (urlParams.get("onboarding") || "").trim();

    function stripStaleOnboardingParams() {
      try {
        var params = new URLSearchParams(window.location.search || "");
        if (!params.has("onboarding")) return;
        var mode = (params.get("onboarding") || "").trim();
        if (needsConsent && mode === "consent") return;
        if (needsProfile && mode === "profile") return;
        params.delete("onboarding");
        var q = params.toString();
        var clean =
          window.location.pathname + (q ? "?" + q : "") + (window.location.hash || "");
        if (window.history && window.history.replaceState) {
          window.history.replaceState(window.history.state || {}, "", clean);
        }
        onboardingMode = "";
      } catch (_) {}
    }

    stripStaleOnboardingParams();
    var selfPrivateHandle = (shell.getAttribute("data-self-private-handle") || "").trim();
    var feedDefaultTpl = document.getElementById("app-home-feed-default-template");
    var currentFeedPage = 0;
    var FEED_PAGE_MIN = 0;
    var FEED_PAGE_MAX = 100000;
    function clampFeedPage(raw) {
      var n = Number(raw);
      if (!Number.isFinite(n)) n = 0;
      n = Math.trunc(n);
      if (n < FEED_PAGE_MIN) return FEED_PAGE_MIN;
      if (n > FEED_PAGE_MAX) return FEED_PAGE_MAX;
      return n;
    }

    var pendingFeedScrollToTop = false;

    function scrollFeedToTop(smooth) {
      var behavior = smooth ? "smooth" : "auto";
      if (appMain && typeof appMain.scrollTo === "function") {
        appMain.scrollTo({ top: 0, left: 0, behavior: behavior });
        return;
      }
      if (window && typeof window.scrollTo === "function") {
        window.scrollTo({ top: 0, left: 0, behavior: behavior });
      }
    }

    function scrollFeedAfterPageLoad() {
      if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(function () {
          requestAnimationFrame(function () {
            scrollFeedToTop(true);
          });
        });
        return;
      }
      scrollFeedToTop(true);
    }

    var feedScrollTopBtn = null;

    function feedScrollTopAlignTarget() {
      var nextBtn = document.querySelector('[data-feed-nav="next"]');
      if (nextBtn && !nextBtn.hidden) return nextBtn;
      var endBtn = document.querySelector(".app-rail-pinned-end .app-rail-feed-nav-btn");
      if (endBtn && !endBtn.hidden) return endBtn;
      return (
        document.getElementById("app-topbar-create-btn") ||
        document.querySelector(".app-topbar-action-cell")
      );
    }

    function syncFeedScrollTopButtonOffset() {
      if (!feedScrollTopBtn) return;
      var footer = document.querySelector(".public-footer");
      var gapPx = 8;
      if (footer) {
        feedScrollTopBtn.style.bottom = footer.offsetHeight + gapPx + "px";
      } else {
        feedScrollTopBtn.style.bottom = "";
      }
      var alignBtn = feedScrollTopAlignTarget();
      if (alignBtn && alignBtn.getBoundingClientRect) {
        var rect = alignBtn.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          feedScrollTopBtn.style.left = rect.left + "px";
          feedScrollTopBtn.style.right = "auto";
          feedScrollTopBtn.style.width = rect.width + "px";
          feedScrollTopBtn.style.height = rect.height + "px";
          feedScrollTopBtn.style.minHeight = rect.height + "px";
        }
      }
    }

    function syncFeedScrollTopButton() {
      if (!feedScrollTopBtn) return;
      var feedVisible =
        feedPanel && !feedPanel.hidden && !!document.getElementById("app-feed-cards-wrap");
      var scrolled = appMain && appMain.scrollTop > 120;
      var show = feedVisible && scrolled;
      feedScrollTopBtn.hidden = !show;
      if (show) feedScrollTopBtn.removeAttribute("hidden");
      else feedScrollTopBtn.setAttribute("hidden", "hidden");
      if (show) syncFeedScrollTopButtonOffset();
    }

    var FEED_CHROME_NAV_SPRITE =
      "/static/icons/post-actions.svg?v=20260812-chrome-nav";

    function ensureFeedScrollTopButton() {
      if (!appMain || feedScrollTopBtn) return;
      var btn = document.createElement("button");
      btn.type = "button";
      btn.id = "app-feed-scroll-top-btn";
      btn.className =
        "app-rail-slot app-rail-feed-nav-btn app-chrome-nav-btn app-feed-scroll-top-btn system-icon-btn";
      btn.hidden = true;
      btn.setAttribute(
        "aria-label",
        (UI && UI.feed_scroll_to_top) || "Back to top of feed"
      );
      btn.innerHTML =
        '<span class="app-rail-slot-frame app-rail-slot-frame--action app-rail-slot-frame--feed-nav-action" aria-hidden="true">' +
        '<span class="app-rail-action-disc">' +
        '<svg class="app-rail-feed-nav-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">' +
        '<use href="' +
        FEED_CHROME_NAV_SPRITE +
        '#icon-feed-top"/></svg></span></span>';
      btn.addEventListener("click", function () {
        scrollFeedToTop(true);
      });
      document.body.appendChild(btn);
      feedScrollTopBtn = btn;
      syncFeedScrollTopButtonOffset();
      window.addEventListener("resize", syncFeedScrollTopButtonOffset, { passive: true });
      appMain.addEventListener("scroll", syncFeedScrollTopButton, { passive: true });
    }
    var initialDocumentTitle = document.title;
    var topbarTitleEl0 = document.querySelector(".app-topbar-title");
    var initialTopbarTitleHtml = topbarTitleEl0 ? topbarTitleEl0.innerHTML : "";

    function bindCreatePostComposer(UI) {
      var form = document.getElementById("create-post-form");
      if (!form || form.getAttribute("data-create-bound") === "1") return;
      form.setAttribute("data-create-bound", "1");
      /** First http(s) URL we last fetched OG for; refetch only when this changes. */
      var ogPreviewActiveUrl = "";
      var audienceInput = document.getElementById("create-post-audience");
      var AUDIENCE_PREFIX = "For: ";
      var AUDIENCE_DEFAULT_HANDLES = "@friends, @family, @colleagues, @private";
      var AUDIENCE_BRAND_HANDLES = "@base";
      var extractAudienceHandles = function (raw) {
        var s = String(raw || "");
        s = s.replace(/^for:\s*/i, "");
        var matches = s.match(/@[A-Za-z0-9_]+/g) || [];
        return matches.map(function (h) {
          return h.toLowerCase();
        });
      };
      var normalizeAudienceHandles = function (raw) {
        if (isEntityScopedAudienceType()) {
          var scopedHandle = selectedPostAsHandle();
          return AUDIENCE_PREFIX + (scopedHandle ? "@" + scopedHandle : "");
        }
        var handles = extractAudienceHandles(raw);
        // Keep aliases in sync with backend parser.
        handles = handles.map(function (h) {
          if (h === "@followers") return "@base";
          return h;
        });
        var tk = selectedPostAsTypeKey();
        var lastPub = -1;
        var lastPriv = -1;
        handles.forEach(function (h, i) {
          if (h === "@public") lastPub = i;
          if (h === "@private") lastPriv = i;
        });
        if (lastPub !== -1 && lastPriv !== -1) {
          if (lastPub > lastPriv) {
            handles = handles.filter(function (h) {
              return h !== "@private";
            });
          } else {
            handles = handles.filter(function (h) {
              return h !== "@public";
            });
          }
        }
        if ((tk !== "profit" && tk !== "non_profit") && !handles.some(function (h) { return h === "@public" || h === "@private"; })) {
          handles.push("@private");
        }
        return AUDIENCE_PREFIX + handles.join(", ");
      };
      // Remove any stale legacy title/label above audience input.
      var legacyAudienceLabel = form.querySelector(
        ".create-post-to-label, label[for='create-post-audience'], .create-post-to-row"
      );
      if (legacyAudienceLabel) legacyAudienceLabel.remove();

      var entityHidden = document.getElementById("create-post-entity");
      var entityPickerWrap = document.getElementById("create-post-entity-picker");
      var entityTrigger = document.getElementById("create-post-entity-trigger");
      var entityMenu = document.getElementById("create-post-entity-menu");
      var entityPickerRows = [];
      var entityPickerIndex = 0;
      var entityPickerOpen = false;

      var createPostNomAmountHtml = function (amount, unaffordable) {
        var icon = UI.nom_icon_sm || "";
        var cls = "nom-amount" + (unaffordable ? " nom-amount--insufficient" : "");
        return (
          '<span class="' +
          cls +
          '"><span class="nom-amount-count mono">' +
          String(amount) +
          "</span>" +
          icon +
          "</span>"
        );
      };

      var createPostEntityUnaffordable = function (row) {
        if (!row) return false;
        return row.can_afford_post === false;
      };

      var createPostEntityDisplayName = function (row) {
        if (!row) return "";
        var handle = String(row.handle || "").trim();
        var label = String(row.label || "").trim();
        return handle || label;
      };

      var createPostEntityOptionHtml = function (row) {
        var name = createPostEntityDisplayName(row);
        var cost = Number(row.post_nom_cost || 0);
        if (!cost) return escapeHtml(name);
        return escapeHtml(name) + " " + createPostNomAmountHtml(cost, createPostEntityUnaffordable(row));
      };

      var createPostEntityOptionPlain = function (row) {
        var name = createPostEntityDisplayName(row);
        var cost = Number(row.post_nom_cost || 0);
        if (!cost) return name;
        var suffix = createPostEntityUnaffordable(row) ? " (insufficient Noms)" : "";
        return name + " " + String(cost) + " Noms" + suffix;
      };

      var parsePostingEntitiesPayload = function (payload) {
        if (Array.isArray(payload)) {
          return { wallet_balance: null, entities: payload };
        }
        return {
          wallet_balance:
            payload && payload.wallet_balance != null ? Number(payload.wallet_balance) : null,
          entities: payload && Array.isArray(payload.entities) ? payload.entities : [],
        };
      };

      var firstAffordablePostingEntityIndex = function (rows) {
        for (var i = 0; i < rows.length; i++) {
          if (!createPostEntityUnaffordable(rows[i])) return i;
        }
        return rows.length > 0 ? 0 : -1;
      };

      var closeCreatePostEntityMenu = function () {
        if (!entityMenu || !entityTrigger) return;
        entityMenu.hidden = true;
        entityPickerOpen = false;
        entityTrigger.setAttribute("aria-expanded", "false");
      };

      var openCreatePostEntityMenu = function () {
        if (!entityMenu || !entityTrigger) return;
        entityMenu.hidden = false;
        entityPickerOpen = true;
        entityTrigger.setAttribute("aria-expanded", "true");
      };

      var syncCreatePostEntitySelection = function () {
        var row = entityPickerRows[entityPickerIndex];
        if (!row) return;
        if (entityHidden) entityHidden.value = String(row.id || "");
        if (entityTrigger) {
          var labelEl = entityTrigger.querySelector(".create-post-entity-trigger-label");
          if (labelEl) labelEl.innerHTML = createPostEntityOptionHtml(row);
        }
        if (entityMenu) {
          entityMenu.querySelectorAll('[role="option"]').forEach(function (opt, idx) {
            opt.setAttribute("aria-selected", idx === entityPickerIndex ? "true" : "false");
          });
        }
      };

      var selectedPostAsTypeKey = function () {
        var row = entityPickerRows[entityPickerIndex];
        return String((row && row.type_key) || "").trim();
      };
      var selectedPostAsHandle = function () {
        var row = entityPickerRows[entityPickerIndex];
        return normalizeHandleCandidate(
          String((row && row.handle) || createPostEntityDisplayName(row) || "")
        );
      };
      var isEntityScopedAudienceType = function () {
        var tk = selectedPostAsTypeKey();
        return tk === "community" || tk === "gathering";
      };
      var ogStatus = document.getElementById("create-post-og-status");
      var submitStatus = document.getElementById("create-post-submit-status");
      var ogPreview = document.getElementById("create-post-og-preview");
      var editor = document.getElementById("create-post-editor");
      var charHint = document.getElementById("create-post-char-hint");
      var backBtn = document.getElementById("create-post-back");
      var toolbar = form.querySelector(".create-post-editor-toolbar");
      // OG preview: below audience / hint, above the editor (toolbar is below editor; fixes stale fragments).
      if (editor && ogStatus && ogPreview) {
        try {
          editor.parentNode.insertBefore(ogStatus, editor);
          editor.parentNode.insertBefore(ogPreview, editor);
        } catch (_) {}
      }
      // Formatting row sits above editor; counter sits below editor.
      if (toolbar && editor) {
        try {
          if (toolbar.nextElementSibling !== editor) {
            editor.parentNode.insertBefore(toolbar, editor);
          }
        } catch (_) {}
      }
      if (editor && charHint) {
        try {
          if (charHint.previousElementSibling !== editor) {
            editor.parentNode.insertBefore(charHint, editor.nextSibling);
          }
        } catch (_) {}
      }
      if (toolbar) {
        var colorBtns = toolbar.querySelectorAll(".create-post-editor-color-btn");
        colorBtns.forEach(function (btn) {
          // Back-compat: older fragments still ship W/Y/B/... glyphs.
          btn.textContent = "";
          var swatch = (btn.getAttribute("data-value") || "").trim();
          if (swatch) btn.style.setProperty("--swatch", swatch);
        });
      }
      var CREATE_POST_MAX_CHARS = 4000;

      var editorMarkupLength = function () {
        if (!editor) return 0;
        return String(editor.innerHTML || "").replace(/\r\n/g, "\n").length;
      };
      var charHintText = function (used) {
        return String(used) + "/" + String(CREATE_POST_MAX_CHARS);
      };
      var updateCharHint = function () {
        if (!charHint || !editor) return;
        var used = Math.max(0, editorMarkupLength());
        charHint.textContent = charHintText(Math.min(CREATE_POST_MAX_CHARS, used));
        charHint.classList.toggle("is-limit", used >= CREATE_POST_MAX_CHARS);
      };
      if (!charHint && editor) {
        // Back-compat for stale fragments: create counter hint if missing in HTML.
        charHint = document.createElement("p");
        charHint.id = "create-post-char-hint";
        charHint.className = "hint create-post-char-hint";
        editor.insertAdjacentElement("afterend", charHint);
      }
      var enforceEditorCharLimit = function () {
        if (!editor || editorMarkupLength() <= CREATE_POST_MAX_CHARS) return;
        // Prefer undo so rich-text formatting survives the last invalid insert.
        try {
          document.execCommand("undo", false, null);
        } catch (_) {}
        if (editorMarkupLength() > CREATE_POST_MAX_CHARS) {
          // Hard fallback: trim visible text if undo did not recover a <= 4000 markup payload.
          editor.textContent = String(editor.innerText || "").replace(/\r\n/g, "\n").slice(0, CREATE_POST_MAX_CHARS);
        }
      };

      if (audienceInput && !audienceInput.value.trim()) {
        audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_DEFAULT_HANDLES;
      }

      if (entityPickerWrap && entityHidden && entityHidden.tagName !== "SELECT") {
        fetch("/me/posting-entities", {
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        })
          .then(function (r) {
            if (!r.ok) throw new Error("entity_load_failed");
            return r.json();
          })
          .then(function (payload) {
            var parsed = parsePostingEntitiesPayload(payload);
            entityPickerRows = parsed.entities;
            entityPickerIndex = firstAffordablePostingEntityIndex(entityPickerRows);
            if (entityPickerIndex < 0) entityPickerIndex = 0;
            if (entityMenu) {
              entityMenu.innerHTML = "";
              entityPickerRows.forEach(function (row, idx) {
                var li = document.createElement("li");
                li.setAttribute("role", "option");
                li.setAttribute("data-index", String(idx));
                li.setAttribute("tabindex", "-1");
                if (createPostEntityUnaffordable(row)) {
                  li.setAttribute("aria-disabled", "true");
                  li.classList.add("create-post-entity-option--unaffordable");
                }
                li.innerHTML = createPostEntityOptionHtml(row);
                entityMenu.appendChild(li);
              });
            }
            var syncCreatePostCardAccent = function () {
              var card = document.getElementById("create-post-card");
              if (!card) return;
              var tk = selectedPostAsTypeKey();
              if (!tk) return;
              card.className = String(card.className || "")
                .replace(/\bentity-profile-accent--\S+/g, "")
                .replace(/\s+/g, " ")
                .trim();
              card.classList.add("entity-profile-accent--" + tk);
            };
            var applyAudiencePolicyForEntity = function () {
              if (!audienceInput) return;
              var tk = selectedPostAsTypeKey();
              if (tk === "profit" || tk === "non_profit") {
                audienceInput.readOnly = false;
                audienceInput.setAttribute("aria-readonly", "false");
                audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_BRAND_HANDLES;
                return;
              }
              if (tk === "gathering") {
                var h = selectedPostAsHandle();
                audienceInput.readOnly = true;
                audienceInput.setAttribute("aria-readonly", "true");
                audienceInput.value = AUDIENCE_PREFIX + (h ? "@" + h : "");
                return;
              }
              audienceInput.readOnly = false;
              audienceInput.setAttribute("aria-readonly", "false");
              audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_DEFAULT_HANDLES;
            };
            var onCreatePostEntityChanged = function () {
              syncCreatePostEntitySelection();
              syncCreatePostCardAccent();
              applyAudiencePolicyForEntity();
              if (
                createPostMentionTypeahead &&
                typeof createPostMentionTypeahead.refresh === "function"
              ) {
                createPostMentionTypeahead.refresh();
              }
            };
            onCreatePostEntityChanged();
            bindCreatePostTypeaheads();
            applyPendingCreatePostPreset();
            if (entityTrigger) {
              entityTrigger.addEventListener("click", function () {
                if (entityPickerOpen) closeCreatePostEntityMenu();
                else openCreatePostEntityMenu();
              });
            }
            if (entityMenu) {
              entityMenu.addEventListener("click", function (e) {
                var opt = e.target && e.target.closest ? e.target.closest('[role="option"]') : null;
                if (!opt || opt.getAttribute("aria-disabled") === "true") return;
                var idx = Number(opt.getAttribute("data-index"));
                if (!Number.isFinite(idx) || idx < 0 || idx >= entityPickerRows.length) return;
                entityPickerIndex = idx;
                closeCreatePostEntityMenu();
                onCreatePostEntityChanged();
              });
            }
            document.addEventListener("click", function (e) {
              if (!entityPickerOpen || !entityPickerWrap) return;
              if (entityPickerWrap.contains(e.target)) return;
              closeCreatePostEntityMenu();
            });
            document.addEventListener("keydown", function (e) {
              if (!entityPickerOpen) return;
              if (e.key === "Escape") {
                closeCreatePostEntityMenu();
                if (entityTrigger) entityTrigger.focus();
              }
            });
          })
          .catch(function () {
            if (entityTrigger) {
              var labelEl = entityTrigger.querySelector(".create-post-entity-trigger-label");
              if (labelEl) labelEl.textContent = UI.request_failed || "Request failed.";
            }
          });
      } else if (entityHidden && entityHidden.tagName === "SELECT") {
        fetch("/me/posting-entities", {
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        })
          .then(function (r) {
            if (!r.ok) throw new Error("entity_load_failed");
            return r.json();
          })
          .then(function (payload) {
            var parsed = parsePostingEntitiesPayload(payload);
            var rows = parsed.entities;
            entityHidden.innerHTML = "";
            rows.forEach(function (row, idx) {
              var opt = document.createElement("option");
              opt.value = row.id;
              opt.textContent = createPostEntityOptionPlain(row);
              opt.setAttribute("data-handle", String(row.handle || ""));
              if (row.type_key) opt.setAttribute("data-type-key", String(row.type_key));
              if (createPostEntityUnaffordable(row)) opt.disabled = true;
              if (idx === firstAffordablePostingEntityIndex(rows)) opt.selected = true;
              entityHidden.appendChild(opt);
            });
            var syncCreatePostCardAccentLegacy = function () {
              var card = document.getElementById("create-post-card");
              if (!card || !entityHidden) return;
              var opt = entityHidden.options[entityHidden.selectedIndex];
              var tk = (opt && opt.getAttribute("data-type-key")) || "";
              if (!tk) return;
              card.className = String(card.className || "")
                .replace(/\bentity-profile-accent--\S+/g, "")
                .replace(/\s+/g, " ")
                .trim();
              card.classList.add("entity-profile-accent--" + tk);
            };
            var applyAudiencePolicyLegacy = function () {
              if (!audienceInput || !entityHidden) return;
              var opt = entityHidden.options[entityHidden.selectedIndex];
              var tk = String((opt && opt.getAttribute("data-type-key")) || "").trim();
              if (tk === "profit" || tk === "non_profit") {
                audienceInput.readOnly = false;
                audienceInput.setAttribute("aria-readonly", "false");
                audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_BRAND_HANDLES;
                return;
              }
              if (tk === "gathering") {
                var h = normalizeHandleCandidate(String((opt && opt.getAttribute("data-handle")) || ""));
                audienceInput.readOnly = true;
                audienceInput.setAttribute("aria-readonly", "true");
                audienceInput.value = AUDIENCE_PREFIX + (h ? "@" + h : "");
                return;
              }
              audienceInput.readOnly = false;
              audienceInput.setAttribute("aria-readonly", "false");
              audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_DEFAULT_HANDLES;
            };
            syncCreatePostCardAccentLegacy();
            applyAudiencePolicyLegacy();
            bindCreatePostTypeaheads();
            applyPendingCreatePostPresetLegacy();
            entityHidden.addEventListener("change", function () {
              syncCreatePostCardAccentLegacy();
              applyAudiencePolicyLegacy();
            });
          })
          .catch(function () {
            var opt = document.createElement("option");
            opt.textContent = UI.request_failed || "Request failed.";
            entityHidden.innerHTML = "";
            entityHidden.appendChild(opt);
          });
      }

      if (toolbar && editor) {
        toolbar.addEventListener("click", function (e) {
          var btn = e.target && e.target.closest ? e.target.closest("[data-cmd]") : null;
          if (!btn) return;
          e.preventDefault();
          var cmd = btn.getAttribute("data-cmd");
          if (!cmd) return;
          editor.focus();
          var value = btn.getAttribute("data-value");
          document.execCommand(cmd, false, value != null ? value : null);
        });
      }

      if (audienceInput) {
        var keepCaretAfterPrefix = function () {
          if (!audienceInput) return;
          var minPos = AUDIENCE_PREFIX.length;
          var start = audienceInput.selectionStart || 0;
          var end = audienceInput.selectionEnd || 0;
          if (start < minPos || end < minPos) {
            audienceInput.setSelectionRange(minPos, minPos);
          }
        };
        audienceInput.addEventListener("keydown", function (e) {
          if (audienceInput.readOnly) return;
          var minPos = AUDIENCE_PREFIX.length;
          var start = audienceInput.selectionStart || 0;
          var end = audienceInput.selectionEnd || 0;
          var v = audienceInput.value;

          if ((e.key === "Backspace" && start <= minPos) || (e.key === "ArrowLeft" && start <= minPos && end <= minPos)) {
            e.preventDefault();
            audienceInput.setSelectionRange(minPos, minPos);
            return;
          }
          if (e.key === "Delete" && start < minPos) {
            e.preventDefault();
            audienceInput.setSelectionRange(minPos, minPos);
            return;
          }

          // Space: insert " @" or "@" so the user can type the next handle without fighting normalization.
          if (e.key === " " && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
            e.preventDefault();
            var prevCh = start > 0 ? v.slice(start - 1, start) : "";
            if (prevCh === "@") {
              return;
            }
            var insert = prevCh === " " || prevCh === "\t" ? "@" : " @";
            audienceInput.value = v.slice(0, start) + insert + v.slice(end);
            var nextPos = start + insert.length;
            audienceInput.setSelectionRange(nextPos, nextPos);
            return;
          }

          // @: ensure a separating space before @ when not already after whitespace (after "For: " is OK bare @).
          if (e.key === "@" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
            if (start > minPos) {
              var prev = v.slice(start - 1, start);
              if (prev !== " " && prev !== "\t") {
                e.preventDefault();
                var insert = " @";
                audienceInput.value = v.slice(0, start) + insert + v.slice(end);
                var nextPos = start + insert.length;
                audienceInput.setSelectionRange(nextPos, nextPos);
              }
            }
          }
        });
        audienceInput.addEventListener("click", keepCaretAfterPrefix);
        audienceInput.addEventListener("focus", keepCaretAfterPrefix);
        audienceInput.addEventListener("input", function () {
          if (audienceInput.readOnly) return;
          keepCaretAfterPrefix();
        });
        audienceInput.addEventListener("blur", function () {
          if (audienceInput.readOnly) return;
          var next = normalizeAudienceHandles(audienceInput.value);
          if (next !== audienceInput.value) {
            audienceInput.value = next;
          }
          keepCaretAfterPrefix();
        });
        audienceInput.value = normalizeAudienceHandles(audienceInput.value);
        keepCaretAfterPrefix();
      }

      var createPostOgDomainBlocked = false;
      var createPostOgLoading = false;
      var domainIdInput = document.getElementById("create-post-domain-id");
      var linkIdInput = document.getElementById("create-post-link-id");

      /** Longest patterns first so `:-)` wins over `:)` etc. */
      var CREATE_POST_EMOTICONS = [
        { pattern: ":-)", emoji: "😊" },
        { pattern: ":-(", emoji: "😢" },
        { pattern: ":-D", emoji: "😀" },
        { pattern: ":-P", emoji: "😛" },
        { pattern: ":-p", emoji: "😛" },
        { pattern: ":-o", emoji: "😮" },
        { pattern: ":-O", emoji: "😮" },
        { pattern: ":-/", emoji: "😕" },
        { pattern: ";-)", emoji: "😉" },
        { pattern: ":D", emoji: "😀" },
        { pattern: ":d", emoji: "😀" },
        { pattern: ":P", emoji: "😛" },
        { pattern: ":p", emoji: "😛" },
        { pattern: ":o", emoji: "😮" },
        { pattern: ":O", emoji: "😮" },
        { pattern: ":/", emoji: "😕" },
        { pattern: ":|", emoji: "😐" },
        { pattern: ":)", emoji: "😊" },
        { pattern: ":(", emoji: "😢" },
        { pattern: ";)", emoji: "😉" },
        { pattern: "<3", emoji: "❤️" },
        { pattern: "^_^", emoji: "😊" },
      ];

      function walkCharOffsetToNode(root, charIndex) {
        var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
        var pos = 0;
        var n;
        while ((n = walker.nextNode())) {
          var t = n.nodeValue || "";
          var len = t.length;
          if (charIndex <= pos + len) {
            return { node: n, offset: charIndex - pos };
          }
          pos += len;
        }
        return null;
      }

      /**
       * If text ending at the caret matches a known emoticon, replace it with emoji.
       * Skips during IME composition. Returns true if a replacement happened.
       */
      function replaceEmoticonsAtCaret(editor, inputEvent) {
        if (!editor) return false;
        if (inputEvent && inputEvent.isComposing) return false;
        var sel = window.getSelection();
        if (!sel.rangeCount) return false;
        var range = sel.getRangeAt(0);
        if (!range.collapsed) return false;
        if (!editor.contains(range.commonAncestorContainer)) return false;

        var r = document.createRange();
        r.setStart(editor, 0);
        r.setEnd(range.endContainer, range.endOffset);
        var textBefore = r.toString();
        var totalLen = textBefore.length;
        if (totalLen === 0) return false;

        var i;
        var item;
        var p;
        for (i = 0; i < CREATE_POST_EMOTICONS.length; i++) {
          item = CREATE_POST_EMOTICONS[i];
          p = item.pattern;
          if (p.length > totalLen) continue;
          if (textBefore.slice(-p.length) !== p) continue;

          var startCharIndex = totalLen - p.length;
          var startPos = walkCharOffsetToNode(editor, startCharIndex);
          var endPos = walkCharOffsetToNode(editor, totalLen);
          if (!startPos || !endPos) continue;

          var del = document.createRange();
          try {
            del.setStart(startPos.node, startPos.offset);
            del.setEnd(endPos.node, endPos.offset);
          } catch (_) {
            continue;
          }
          del.deleteContents();
          var tn = document.createTextNode(item.emoji);
          del.insertNode(tn);
          sel.removeAllRanges();
          var nr = document.createRange();
          nr.setStartAfter(tn);
          nr.collapse(true);
          sel.addRange(nr);
          return true;
        }
        return false;
      }

      var createPostMentionTypeahead = null;

      var applyPendingCreatePostPreset = function () {
        if (!pendingCreatePostPreset || !pendingCreatePostPreset.privateMessageHandle) return;
        var handle = normalizeHandleCandidate(
          String(pendingCreatePostPreset.privateMessageHandle || "")
        );
        pendingCreatePostPreset = null;
        if (!handle || !audienceInput) return;

        var humanIdx = -1;
        for (var i = 0; i < entityPickerRows.length; i++) {
          if (String(entityPickerRows[i].type_key || "") === "human") {
            humanIdx = i;
            break;
          }
        }
        if (humanIdx >= 0) {
          entityPickerIndex = humanIdx;
          syncCreatePostEntitySelection();
          var card = document.getElementById("create-post-card");
          if (card) {
            card.className = String(card.className || "")
              .replace(/\bentity-profile-accent--\S+/g, "")
              .replace(/\s+/g, " ")
              .trim();
            card.classList.add("entity-profile-accent--human");
          }
        }
        audienceInput.readOnly = false;
        audienceInput.setAttribute("aria-readonly", "false");
        audienceInput.value = normalizeAudienceHandles(
          AUDIENCE_PREFIX + "@private, @" + handle
        );
        if (
          createPostMentionTypeahead &&
          typeof createPostMentionTypeahead.refresh === "function"
        ) {
          createPostMentionTypeahead.refresh();
        }
        if (editor) {
          window.setTimeout(function () {
            editor.focus();
          }, 0);
        }
      };

      var applyPendingCreatePostPresetLegacy = function () {
        if (!pendingCreatePostPreset || !pendingCreatePostPreset.privateMessageHandle) return;
        var handle = normalizeHandleCandidate(
          String(pendingCreatePostPreset.privateMessageHandle || "")
        );
        pendingCreatePostPreset = null;
        if (!handle || !audienceInput || !entityHidden) return;

        for (var i = 0; i < entityHidden.options.length; i++) {
          var opt = entityHidden.options[i];
          if (String(opt.getAttribute("data-type-key") || "") === "human") {
            entityHidden.selectedIndex = i;
            break;
          }
        }
        audienceInput.readOnly = false;
        audienceInput.setAttribute("aria-readonly", "false");
        audienceInput.value = normalizeAudienceHandles(
          AUDIENCE_PREFIX + "@private, @" + handle
        );
        if (editor) {
          window.setTimeout(function () {
            editor.focus();
          }, 0);
        }
      };

      function bindCreatePostTypeaheads() {
        if (editor && window.AppHandleTypeahead && editor.getAttribute("data-mention-typeahead") !== "1") {
          editor.setAttribute("data-mention-typeahead", "1");
          createPostMentionTypeahead = window.AppHandleTypeahead.bindEditor({
            editor: editor,
            fallbackGlobal: true,
            overlayLimit: 3,
            showAllLabel: UI.mention_show_all || "Show all ({count})",
            ariaLabel: UI.mention_autocomplete_aria || "Mention suggestions",
            getContext: function () {
              var entitySel = document.getElementById("create-post-entity");
              return {
                entityId: entitySel ? String(entitySel.value || "").trim() : "",
                audienceHandles: audienceInput ? String(audienceInput.value || "").trim() : "",
              };
            },
          });
        }
        if (
          audienceInput &&
          window.AppHandleTypeahead &&
          audienceInput.getAttribute("data-audience-typeahead") !== "1"
        ) {
          audienceInput.setAttribute("data-audience-typeahead", "1");
          window.AppHandleTypeahead.bindInput({
            input: audienceInput,
            mode: "audience",
            trigger: "at",
            ariaLabel: UI.audience_autocomplete_aria || "Audience handle suggestions",
          });
        }
        if (createPostMentionTypeahead && typeof createPostMentionTypeahead.refresh === "function") {
          createPostMentionTypeahead.refresh();
        }
      }

      if (editor) {
        editor.addEventListener("input", function (e) {
          replaceEmoticonsAtCaret(editor, e);
          enforceEditorCharLimit();
          updateCharHint();
        });
        editor.addEventListener("paste", function () {
          window.setTimeout(function () {
            replaceEmoticonsAtCaret(editor, null);
            enforceEditorCharLimit();
            updateCharHint();
          }, 0);
        });
        updateCharHint();
      }

      bindCreatePostTypeaheads();

      if (editor && ogStatus && ogPreview) {
        var stripUrlTrailingPunctuation = function (url) {
          return String(url || "").replace(/[.,;:!?)]+$/, "");
        };
        var linkifyEditorUrls = function () {
          if (!editor) return;
          var walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT, null);
          var nodes = [];
          var n;
          while ((n = walker.nextNode())) {
            if (!n.nodeValue || !n.nodeValue.trim()) continue;
            var p = n.parentElement;
            if (p && p.closest && p.closest("a")) continue;
            nodes.push(n);
          }
          var re = /https?:\/\/[^\s<>"')\]]+/gi;
          nodes.forEach(function (textNode) {
            var text = textNode.nodeValue || "";
            re.lastIndex = 0;
            var m;
            var i = 0;
            var changed = false;
            var frag = document.createDocumentFragment();
            while ((m = re.exec(text))) {
              var raw = m[0];
              var href = stripUrlTrailingPunctuation(raw);
              if (!href) continue;
              var start = m.index;
              var end = start + raw.length;
              if (start > i) frag.appendChild(document.createTextNode(text.slice(i, start)));
              var trailing = raw.slice(href.length);
              var a = document.createElement("a");
              a.href = href;
              a.textContent = href;
              a.target = "_blank";
              a.rel = "noopener noreferrer nofollow";
              frag.appendChild(a);
              if (trailing) frag.appendChild(document.createTextNode(trailing));
              i = end;
              changed = true;
            }
            if (!changed) return;
            if (i < text.length) frag.appendChild(document.createTextNode(text.slice(i)));
            textNode.parentNode && textNode.parentNode.replaceChild(frag, textNode);
          });
        };
        var normalizeLinkColorMarkup = function () {
          if (!editor) return;
          editor.querySelectorAll("a").forEach(function (a) {
            a.querySelectorAll("*").forEach(function (child) {
              if (child.removeAttribute) child.removeAttribute("color");
              if (!child.style) return;
              child.style.removeProperty("color");
              child.style.removeProperty("background-color");
              child.style.removeProperty("text-decoration-color");
            });
            if (a.removeAttribute) a.removeAttribute("color");
            if (!a.style) return;
            a.style.removeProperty("color");
            a.style.removeProperty("background-color");
            a.style.removeProperty("text-decoration-color");
          });
        };

        /** First http(s) URL in plain text (reading order); strips common trailing punctuation. */
        var firstHttpUrlFromPlainText = function (s) {
          var t = String(s || "");
          var re = /https?:\/\/[^\s<>"')\]]+/gi;
          var m = re.exec(t);
          if (!m) return "";
          return stripUrlTrailingPunctuation(m[0]);
        };

        var ogPreviewTargetUrl = function () {
          return firstHttpUrlFromPlainText(editor.innerText || "");
        };

        var createPostOgLoadingHtml = function () {
          return window.brandCardLoader ? window.brandCardLoader.publicOgLoadingHtml() : "";
        };

        var ogFetchSeq = 0;

        var resetOgUiHidden = function () {
          ogFetchSeq += 1;
          createPostOgLoading = false;
          ogPreviewActiveUrl = "";
          createPostOgDomainBlocked = false;
          if (domainIdInput) domainIdInput.value = "";
          if (linkIdInput) linkIdInput.value = "";
          ogStatus.hidden = true;
          ogStatus.textContent = "";
          ogStatus.classList.remove("create-post-og-status--blocked");
          ogPreview.hidden = true;
          ogPreview.innerHTML = "";
        };

        var fetchOgDebounced = debounce(function () {
          var url = ogPreviewTargetUrl();
          if (!url) {
            resetOgUiHidden();
            return;
          }
          if (url === ogPreviewActiveUrl) {
            return;
          }
          var mySeq = ++ogFetchSeq;
          createPostOgLoading = true;
          // Clear stale hidden ids immediately when preview target changes.
          if (domainIdInput) domainIdInput.value = "";
          if (linkIdInput) linkIdInput.value = "";
          ogStatus.hidden = true;
          ogStatus.classList.remove("create-post-og-status--blocked");
          ogStatus.textContent = "";
          ogPreview.hidden = false;
          ogPreview.innerHTML = createPostOgLoadingHtml();

          fetch("/content-links/preview-og", {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify({ url: url, for_composer: true }),
          })
            .then(function (r) {
              if (!r.ok) throw new Error("og_load_failed");
              return r.json();
            })
            .then(function (data) {
              if (mySeq !== ogFetchSeq) return;
              createPostOgLoading = false;
              if (data.domain_blocked) {
                ogPreviewActiveUrl = url;
                createPostOgDomainBlocked = true;
                if (domainIdInput) domainIdInput.value = "";
                if (linkIdInput) linkIdInput.value = "";
                ogPreview.hidden = true;
                ogPreview.innerHTML = "";
                ogStatus.hidden = false;
                ogStatus.classList.add("create-post-og-status--blocked");
                ogStatus.textContent = UI.create_post_domain_blocked || "";
                return;
              }
              createPostOgDomainBlocked = false;
              if (domainIdInput) {
                var did = data.domain_id != null && String(data.domain_id).trim() !== "";
                domainIdInput.value = did ? String(data.domain_id).trim() : "";
              }
              if (linkIdInput) {
                var lid = data.link_id != null && String(data.link_id).trim() !== "";
                linkIdInput.value = lid ? String(data.link_id).trim() : "";
              }
              ogStatus.classList.remove("create-post-og-status--blocked");
              var title = data.og_title || data.url_canonical || data.url_original;
              var site = data.og_site_name || data.domain || "";
              var img = data.og_image_url || "";
              var canonical = (data.url_canonical || url || "").trim();
              if (!canonical) {
                ogPreviewActiveUrl = url;
                ogPreview.hidden = true;
                ogPreview.innerHTML = "";
                ogStatus.hidden = true;
                ogStatus.textContent = "";
                return;
              }
              if (mySeq !== ogFetchSeq) return;
              var imgBlock = img
                ? '<div class="public-post-og-media"><img class="public-post-og-image" src="' +
                  escapeHtml(img) +
                  '" alt="" loading="lazy" decoding="async" /></div>'
                : '<div class="public-post-og-media" aria-hidden="true"></div>';
              ogPreview.hidden = false;
              ogPreview.innerHTML =
                '<a class="public-post-og-link" href="' +
                escapeHtml(canonical) +
                '" target="_blank" rel="noopener noreferrer" aria-label="' +
                escapeHtml(title || "Open link") +
                '">' +
                '<div class="public-post-og">' +
                imgBlock +
                '<div class="public-post-og-copy"><div class="public-post-og-title">' +
                escapeHtml(title || "") +
                '</div><div class="public-post-og-site">' +
                escapeHtml(site || "") +
                "</div></div></div></a>";
              ogPreviewActiveUrl = url;
              ogStatus.hidden = true;
              ogStatus.textContent = "";
            })
            .catch(function () {
              if (mySeq !== ogFetchSeq) return;
              createPostOgLoading = false;
              ogPreviewActiveUrl = url;
              createPostOgDomainBlocked = false;
              if (domainIdInput) domainIdInput.value = "";
              if (linkIdInput) linkIdInput.value = "";
              ogPreview.hidden = true;
              ogPreview.innerHTML = "";
              ogStatus.classList.remove("create-post-og-status--blocked");
              ogStatus.hidden = false;
              ogStatus.textContent = UI.create_post_og_failed || "Could not load link preview.";
            });
        }, 500);

        editor.addEventListener("input", function () {
          linkifyEditorUrls();
          normalizeLinkColorMarkup();
          fetchOgDebounced();
        });
        editor.addEventListener("paste", function () {
          window.setTimeout(function () {
            linkifyEditorUrls();
            normalizeLinkColorMarkup();
            fetchOgDebounced();
          }, 0);
        });
      }

      var closeCreateCard = function () {
        if (!feedPanel) return;
        var slot = document.getElementById("app-feed-create-slot");
        if (slot) slot.remove();
        pendingFeedScrollToTop = true;
        scrollFeedToTop(false);
        if (window.history && window.history.pushState) {
          window.history.pushState({ panel: "feed" }, "", "/home");
        }
      };
      if (backBtn) {
        backBtn.addEventListener("click", function () {
          closeCreateCard();
        });
      }

      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        if (submitStatus) {
          submitStatus.hidden = true;
          submitStatus.textContent = "";
          submitStatus.classList.remove("create-post-submit-status--error", "create-post-submit-status--success");
        }
        if (editor && editorMarkupLength() > CREATE_POST_MAX_CHARS) {
          window.alert(
            UI.create_post_character_limit_reached || "Post text is at the 4000 character limit."
          );
          updateCharHint();
          return;
        }
        if (createPostOgDomainBlocked) {
          window.alert(UI.create_post_domain_blocked || "That site cannot be used in a post.");
          return;
        }
        if (createPostOgLoading) {
          window.alert(UI.create_post_og_loading || "Still preparing link preview. Please wait a moment.");
          return;
        }
        if (audienceInput) {
          audienceInput.value = normalizeAudienceHandles(audienceInput.value);
        }
        var submitBtn = document.getElementById("create-post-submit");
        if (submitBtn) {
          submitBtn.disabled = true;
          submitBtn.classList.add("is-loading");
        }
        if (submitStatus) {
          submitStatus.hidden = false;
          submitStatus.textContent = "Posting...";
        }
        try {
          var payload = {
            entity_id: entityHidden ? String(entityHidden.value || "").trim() : "",
            audience_handles: audienceInput ? String(audienceInput.value || "").trim() : "",
            body_markup: editor ? String(editor.innerHTML || "") : "",
            domain_id: domainIdInput ? String(domainIdInput.value || "").trim() : "",
            content_link_id: linkIdInput ? String(linkIdInput.value || "").trim() : "",
          };
          var r = await fetch("/posts", {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify(payload),
          });
          var d = await r.json().catch(function () { return {}; });
          if (!r.ok) {
            if (submitStatus) {
              submitStatus.hidden = false;
              submitStatus.classList.add("create-post-submit-status--error");
              var errMsg =
                (d && d.error) || ("Could not publish post (" + String(r.status) + ").");
              if (
                String(errMsg).indexOf("nom-amount") >= 0 ||
                String(errMsg).indexOf("Nom") >= 0
              ) {
                submitStatus.innerHTML = errMsg;
              } else {
                submitStatus.textContent = errMsg;
              }
            } else {
              window.alert((d && d.error) || UI.request_failed || "Request failed.");
            }
            return;
          }
          if (editor) editor.innerHTML = "";
          if (audienceInput) audienceInput.value = AUDIENCE_PREFIX + AUDIENCE_DEFAULT_HANDLES;
          if (domainIdInput) domainIdInput.value = "";
          if (linkIdInput) linkIdInput.value = "";
          ogPreviewActiveUrl = "";
          if (ogPreview) {
            ogPreview.hidden = true;
            ogPreview.innerHTML = "";
          }
          if (ogStatus) {
            ogStatus.hidden = true;
            ogStatus.textContent = "";
            ogStatus.classList.remove("create-post-og-status--blocked");
          }
          createPostOgDomainBlocked = false;
          updateCharHint();
          closeCreateCard();
          showHomeFeedPanel();
          enqueueRightPanelNotification(
            UI.create_post_success || "Post published.",
            2800
          );
        } catch (_) {
          if (submitStatus) {
            submitStatus.hidden = false;
            submitStatus.classList.add("create-post-submit-status--error");
            submitStatus.textContent = UI.network_error_detail || "Network error. Is the server running?";
          } else {
            window.alert(UI.network_error_detail || "Network error. Is the server running?");
          }
        } finally {
          if (submitBtn) {
            submitBtn.disabled = false;
            submitBtn.classList.remove("is-loading");
          }
        }
      });
    }

    function defaultFeedHtml() {
      if (feedDefaultTpl && typeof feedDefaultTpl.innerHTML === "string" && feedDefaultTpl.innerHTML.trim() !== "") {
        return feedDefaultTpl.innerHTML;
      }
      return "";
    }

    function fetchSearchAddCardHtml() {
      return fetch(SEARCH_ADD_CARD_URL, {
        credentials: "same-origin",
        headers: { Accept: "text/html" },
      }).then(function (r) {
        if (r.status === 429) {
          return r.json().then(function (d) {
            var err = new Error("rate_limited");
            err.rateMsg = (d && d.error) || UI.card_rate_limited;
            throw err;
          });
        }
        if (!r.ok) throw new Error("load_failed");
        return r.text();
      });
    }

    function reloadSearchAddPanel(slot) {
      if (!slot) return Promise.reject(new Error("no_slot"));
      slot.innerHTML = panelCardLoaderHtml("home-search-add");
      return fetchSearchAddCardHtml().then(function (html) {
        slot.innerHTML = html;
        bindSearchAddCard(UI);
      });
    }

    function openCreateEntityCard(createType, slot) {
      if (!slot || !createType) return Promise.resolve();
      slot.innerHTML = panelCardLoaderHtml("home-create-entity");
      return fetch(CREATE_ENTITY_CARD_URL + "?type=" + encodeURIComponent(createType), {
        credentials: "same-origin",
        headers: { Accept: "text/html" },
      })
        .then(function (r) {
          if (r.status === 429) {
            return r.json().then(function (d) {
              var err = new Error("rate_limited");
              err.rateMsg = (d && d.error) || UI.card_rate_limited;
              throw err;
            });
          }
          if (!r.ok) throw new Error("load_failed");
          return r.text();
        })
        .then(function (html) {
          slot.innerHTML = html;
          bindCreateEntityCard(UI, slot);
        });
    }

    var railSearchQueryBound = false;
    var searchAddQuerySeq = 0;
    var railSearchAnimTimer = 0;
    var RAIL_SEARCH_ANIM_MS = 340;

    function getRailSearchControl() {
      return document.getElementById("app-rail-search-control");
    }

    function getRailSearchInput() {
      return document.getElementById("app-rail-search-input");
    }

    function setRailSearchActive(active) {
      var control = getRailSearchControl();
      var wrap = document.getElementById("app-rail-search-input-wrap");
      var btn = document.getElementById("app-rail-search-create-btn");
      var input = getRailSearchInput();
      if (!control) return;

      if (railSearchAnimTimer) {
        window.clearTimeout(railSearchAnimTimer);
        railSearchAnimTimer = 0;
      }

      if (active) {
        if (wrap) wrap.hidden = false;
        window.requestAnimationFrame(function () {
          control.classList.add("is-active");
          if (btn) btn.setAttribute("aria-expanded", "true");
        });
        return;
      }

      control.classList.remove("is-active");
      if (btn) btn.setAttribute("aria-expanded", "false");
      if (input) {
        input.blur();
        input.value = "";
      }
      railSearchAnimTimer = window.setTimeout(function () {
        railSearchAnimTimer = 0;
        if (!control.classList.contains("is-active") && wrap) {
          wrap.hidden = true;
        }
      }, RAIL_SEARCH_ANIM_MS);
    }

    function bindRailSearchClose() {
      var closeBtn = document.getElementById("app-rail-search-close-btn");
      if (!closeBtn || closeBtn.getAttribute("data-rail-search-close-bound") === "1") return;
      closeBtn.setAttribute("data-rail-search-close-bound", "1");
      closeBtn.addEventListener("click", function (e) {
        e.preventDefault();
        e.stopPropagation();
        closeSearchAddPanel();
      });
    }

    function focusRailSearchInput() {
      var input = getRailSearchInput();
      if (!input) return;
      window.requestAnimationFrame(function () {
        try {
          input.focus({ preventScroll: true });
        } catch (_) {
          input.focus();
        }
      });
    }

    function shouldTriggerEntitySearch(raw) {
      var q = String(raw || "").trim().slice(0, 32);
      if (!q) return false;
      if (q.charAt(0) === "@") {
        var body = q.slice(1);
        var alnum = (body.match(/[A-Za-z0-9]/g) || []).length;
        return alnum >= 3;
      }
      return q.length >= 3;
    }

    function renderSearchAddEmptyState(resultsEl, UI) {
      if (!resultsEl) return;
      resultsEl.innerHTML =
        '<li class="search-add-empty-state" id="search-add-empty-state">' +
        escapeHtml(UI.search_no_results || "No results found") +
        "</li>";
    }

    function renderSearchAddLoadingState(resultsEl) {
      if (!resultsEl) return;
      var loader = window.brandCardLoader ? window.brandCardLoader.cardLoaderHtml() : "";
      resultsEl.innerHTML =
        '<li class="search-add-results-loading" role="status" aria-live="polite" aria-busy="true">' +
        loader +
        "</li>";
    }

    function avatarSrcFromStored(stored) {
      var t = String(stored || "").trim();
      if (!t) return "";
      if (/^https?:\/\//i.test(t)) return t;
      if (t.charAt(0) === "/") return t;
      return "/" + t.replace(/^\/+/, "");
    }

    function searchAddResultAvatarHtml(avatarPath, typeKey) {
      if (window.AppAvatarHtml && window.AppAvatarHtml.searchResultAvatarHtml) {
        return window.AppAvatarHtml.searchResultAvatarHtml(avatarPath, typeKey);
      }
      return "";
    }

    function entityTypeAccentClass(typeKey) {
      var t = String(typeKey || "").trim();
      if (t === "human") return "entity-profile-accent--human";
      if (t === "digital") return "entity-profile-accent--digital";
      if (t === "community") return "entity-profile-accent--community";
      if (t === "non_profit") return "entity-profile-accent--non_profit";
      if (t === "profit") return "entity-profile-accent--profit";
      if (t === "gathering") return "entity-profile-accent--gathering";
      return "";
    }

    function renderSearchAddResultRow(it, opts) {
      if (window.AppEntityRecommendations && window.AppEntityRecommendations.renderSearchResultRow) {
        return window.AppEntityRecommendations.renderSearchResultRow(it, opts);
      }
      opts = opts || {};
      var handle = String((it && it.handle) || "")
        .trim()
        .replace(/^@+/, "");
      var showHandle = "@" + handle;
      var label = String((it && it.label) || showHandle).trim();
      var accent = entityTypeAccentClass(it && it.type_key);
      var itemClass = "search-add-result-item" + (accent ? " " + accent : "");
      var inner;
      if (opts.asButton) {
        inner =
          '<button type="button" class="search-add-result-link ' +
          escapeHtml(opts.buttonClass || "") +
          '" data-handle="' +
          escapeHtml(handle) +
          '">' +
          escapeHtml(showHandle) +
          "</button>";
      } else {
        inner =
          '<a class="feed-profile-link search-add-result-link" href="/home/profile/' +
          encodeURIComponent(handle) +
          '" data-profile-handle="' +
          escapeHtml(handle) +
          '">' +
          escapeHtml(showHandle) +
          "</a>";
      }
      return (
        '<li class="' +
        itemClass +
        '">' +
        searchAddResultAvatarHtml(it && it.avatar_path, it && it.type_key) +
        inner +
        '<span class="search-add-result-sub">' +
        escapeHtml(label) +
        "</span>" +
        "</li>"
      );
    }

    function executeSearchAddQuery(UI) {
      var input = getRailSearchInput();
      var results = document.getElementById("search-add-results");
      var status = document.getElementById("search-add-status");
      if (!input || !results) return;
      var q = String(input.value || "").trim().slice(0, 32);
      if (input.value.length > 32) input.value = q;
      if (status) {
        status.hidden = true;
        status.textContent = "";
        status.classList.remove("create-post-submit-status--error");
      }
      if (!shouldTriggerEntitySearch(q)) {
        renderSearchAddResults(results, [], false, UI);
        return;
      }
      var querySeq = ++searchAddQuerySeq;
      renderSearchAddLoadingState(results);
      scrollFeedToTop(false);
      fetch(SEARCH_ENTITIES_URL + "?q=" + encodeURIComponent(q), {
        credentials: "same-origin",
        headers: { Accept: "application/json" },
      })
        .then(function (r) {
          if (r.status === 429) {
            return r.json().then(function (d) {
              var err = new Error("rate_limited");
              err.rateMsg = (d && d.error) || UI.card_rate_limited;
              throw err;
            });
          }
          if (!r.ok) throw new Error("search_failed");
          return r.json();
        })
        .then(function (items) {
          if (querySeq !== searchAddQuerySeq) return;
          renderSearchAddResults(results, items, true, UI);
        })
        .catch(function (e) {
          if (querySeq !== searchAddQuerySeq) return;
          renderSearchAddResults(results, [], false, UI);
          if (status) {
            status.hidden = false;
            status.textContent = (e && e.rateMsg) || UI.search_add_search_failed || "Search failed.";
            status.classList.add("create-post-submit-status--error");
          }
        });
    }

    function renderSearchAddResults(resultsEl, items, queryActive, UI) {
      if (!resultsEl) return;
      var rows = Array.isArray(items) ? items : [];
      if (!rows.length) {
        renderSearchAddEmptyState(resultsEl, UI);
        return;
      }
      resultsEl.innerHTML = rows
        .map(function (it) {
          return renderSearchAddResultRow(it);
        })
        .join("");
    }

    function resumeSearchAddPanelFromRail() {
      if (!feedPanel) return;
      var control = getRailSearchControl();
      if (!control || !control.classList.contains("is-active")) return;

      var profileSlot = document.getElementById("app-private-profile-card-slot");
      var searchSlot = document.getElementById("app-feed-search-add-slot");
      if (!profileSlot && activeView === "feed-search-add" && searchSlot) {
        scrollFeedToTop(false);
        executeSearchAddQuery(UI);
        return;
      }

      if (profileSlot) {
        removePrivateProfileCardSlot();
        currentProfileHandle = "";
      }
      removePrivateProfileLoader();

      if (!searchSlot) {
        if (activeView === "feed-search-add") return;
        openSearchAddPanel();
        return;
      }

      beginView("feed-search-add");
      profilePanel.hidden = true;
      if (createPanel) createPanel.hidden = true;
      feedPanel.hidden = false;
      feedPanel.insertBefore(searchSlot, feedPanel.firstChild);
      scrollFeedToTop(false);
      executeSearchAddQuery(UI);

      var topbarTitle = document.querySelector(".app-topbar-title");
      if (topbarTitle && initialTopbarTitleHtml) topbarTitle.innerHTML = initialTopbarTitleHtml;
      if (initialDocumentTitle) document.title = initialDocumentTitle;
      if (window.history && window.history.pushState) {
        window.history.pushState({ panel: "feed-search-add" }, "", "/home");
      }
    }

    function bindRailSearchQuery(UI) {
      var input = getRailSearchInput();
      if (!input || railSearchQueryBound) return;
      railSearchQueryBound = true;
      input.addEventListener("focus", function () {
        resumeSearchAddPanelFromRail();
      });
      var runSearch = debounce(function () {
        executeSearchAddQuery(UI);
      }, 220);
      input.addEventListener("input", runSearch);
    }

    function closeSearchAddPanel() {
      var slot = document.getElementById("app-feed-search-add-slot");
      if (slot) slot.remove();
      setRailSearchActive(false);
      beginView("feed");
      pendingFeedScrollToTop = true;
      scrollFeedToTop(false);
      if (window.history && window.history.pushState) {
        window.history.pushState({ panel: "feed" }, "", "/home");
      }
    }

    function bindSearchAddCard(UI) {
      var form = document.getElementById("search-add-form");
      if (!form || form.getAttribute("data-search-add-bound") === "1") return;
      form.setAttribute("data-search-add-bound", "1");
      var results = document.getElementById("search-add-results");
      var status = document.getElementById("search-add-status");
      var backBtn = document.getElementById("search-add-back");
      var suggestBtns = form.querySelectorAll(".search-add-suggest-btn");

      function showStatus(msg, isError) {
        if (!status) return;
        status.hidden = !msg;
        status.textContent = msg || "";
        status.classList.toggle("create-post-submit-status--error", !!isError);
      }

      renderSearchAddResults(results, [], false, UI);
      bindRailSearchQuery(UI);
      if (window.AppEntityRecommendations && window.AppEntityRecommendations.loadRecommendationsPanel) {
        window.AppEntityRecommendations.loadRecommendationsPanel(UI);
      }

      executeSearchAddQuery(UI);

      if (backBtn) {
        backBtn.addEventListener("click", function () {
          closeSearchAddPanel();
        });
      }

      suggestBtns.forEach(function (btn) {
        btn.addEventListener("click", function () {
          var createType = String(btn.getAttribute("data-create-type") || "").trim();
          if (!createType) return;
          var slot = document.getElementById("app-feed-search-add-slot");
          showStatus("", false);
          openCreateEntityCard(createType, slot).catch(function (err) {
            showStatus((err && err.rateMsg) || UI.create_entity_load_failed || UI.request_failed || "Request failed.", true);
            if (slot) {
              reloadSearchAddPanel(slot).catch(function () {});
            }
          });
        });
      });

      var inviteForm = document.getElementById("search-add-invite-form");
      if (inviteForm && inviteForm.getAttribute("data-invite-bound") !== "1") {
        inviteForm.setAttribute("data-invite-bound", "1");
        var inviteEmail = document.getElementById("search-add-invite-email");
        var inviteMessage = document.getElementById("search-add-invite-message");
        var inviteSend = document.getElementById("search-add-invite-send");
        var inviteStatus = document.getElementById("search-add-invite-status");
        var inviteSending = false;

        function showInviteStatus(msg, isError, html) {
          if (!inviteStatus) return;
          var hasContent = !!(msg || html);
          inviteStatus.hidden = !hasContent;
          inviteStatus.classList.toggle("create-post-submit-status--error", !!isError);
          if (html) {
            inviteStatus.innerHTML = html;
          } else {
            inviteStatus.innerHTML = "";
            inviteStatus.textContent = msg || "";
          }
        }

        function isFriendInviteEmailValid(raw) {
          var email = String(raw || "").trim();
          return email.length >= 3 && email.length <= 254 && email.indexOf("@") !== -1;
        }

        function syncInviteSendEnabled() {
          var valid = !inviteSending && isFriendInviteEmailValid(inviteEmail && inviteEmail.value);
          if (inviteSend) inviteSend.disabled = !valid;
          if (inviteMessage) inviteMessage.hidden = !isFriendInviteEmailValid(inviteEmail && inviteEmail.value);
        }

        if (inviteEmail) {
          inviteEmail.addEventListener("input", syncInviteSendEnabled);
          inviteEmail.addEventListener("change", syncInviteSendEnabled);
        }
        syncInviteSendEnabled();

        inviteForm.addEventListener("submit", function (e) {
          e.preventDefault();
          var email = inviteEmail ? String(inviteEmail.value || "").trim() : "";
          var personalMessage = inviteMessage ? String(inviteMessage.value || "").trim() : "";
          if (!isFriendInviteEmailValid(email)) {
            showInviteStatus(
              UI.friend_invite_invalid_email || "Enter an email address to invite.",
              true
            );
            syncInviteSendEnabled();
            return;
          }
          inviteSending = true;
          syncInviteSendEnabled();
          showInviteStatus("", false);
          fetch("/me/friend-invite", {
            method: "POST",
            credentials: "same-origin",
            headers: mutationHeaders({ "Content-Type": "application/json" }),
            body: JSON.stringify({ email: email, personal_message: personalMessage }),
          })
            .then(function (r) {
              return r.json().catch(function () { return {}; }).then(function (payload) {
                if (!r.ok) {
                  var err = new Error(
                    (payload && payload.error) ||
                      UI.friend_invite_failed ||
                      "Could not send that invitation right now."
                  );
                  err.rateMsg = err.message;
                  throw err;
                }
                return payload;
              });
            })
            .then(function (payload) {
              if (payload && payload.already_member) {
                var handle = String((payload && payload.handle) || "")
                  .trim()
                  .replace(/^@+/, "");
                if (handle) {
                  var tmpl =
                    UI.friend_invite_already_on_platform || "{handle} is already on 256.";
                  var link =
                    '<a class="feed-profile-link search-add-invite-existing-link" href="/home/profile/' +
                    encodeURIComponent(handle) +
                    '" data-profile-handle="' +
                    escapeHtml(handle) +
                    '">@' +
                    escapeHtml(handle) +
                    "</a>";
                  var parts = tmpl.split("{handle}");
                  showInviteStatus("", false, escapeHtml(parts[0] || "") + link + escapeHtml(parts[1] || ""));
                } else {
                  showInviteStatus(
                    (payload && payload.message) ||
                      UI.friend_invite_email_registered ||
                      "That email is already registered.",
                    false
                  );
                }
                if (inviteEmail) inviteEmail.value = "";
                if (inviteMessage) inviteMessage.value = "";
                return;
              }
              if (inviteEmail) inviteEmail.value = "";
              if (inviteMessage) inviteMessage.value = "";
              showInviteStatus(
                (payload && payload.message) || UI.friend_invite_sent || "Invitation sent.",
                false
              );
            })
            .catch(function (err) {
              showInviteStatus(
                (err && err.rateMsg) ||
                  UI.friend_invite_failed ||
                  "Could not send that invitation right now.",
                true
              );
            })
            .finally(function () {
              inviteSending = false;
              syncInviteSendEnabled();
            });
        });
      }
    }

    function bindCreateEntityCard(UI, slot) {
      var form = document.getElementById("create-entity-form");
      if (!form || form.getAttribute("data-create-entity-bound") === "1") return;
      form.setAttribute("data-create-entity-bound", "1");
      var card = form.closest ? form.closest("#create-entity-card") : null;
      var isProfit = form.classList.contains("create-entity-form--profit");
      var entityTypeKey = (
        form.getAttribute("data-entity-type-key") ||
        (card && card.getAttribute ? card.getAttribute("data-entity-type-key") : "") ||
        ""
      ).trim();
      var isNonProfit = form.classList.contains("create-entity-form--non-profit") || entityTypeKey === "non_profit";
      var isGathering = form.classList.contains("create-entity-form--gathering") || entityTypeKey === "gathering";
      var isCommunity = form.classList.contains("create-entity-form--community") || entityTypeKey === "community";
      var backBtn = document.getElementById("create-entity-back");
      var status = document.getElementById("create-entity-status");
      var locInput = document.getElementById("create-entity-location-id");
      var warn = document.getElementById("create-entity-location-warning");
      var submitBtn = document.getElementById("create-entity-submit");
      var handleInput = document.getElementById("create-entity-handle");
      var placeIdInput = document.getElementById("create-entity-google-place-id");
      var placeQueryInput = document.getElementById("create-entity-google-place-query");
      var placeResults = document.getElementById("create-entity-place-results");
      var placeBadge = document.getElementById("create-entity-place-badge");
      var descriptionInput = document.getElementById("create-entity-description");
      var descriptionEditor = document.getElementById("create-entity-description-editor");
      var descriptionToolbar = form.querySelector(".create-entity-description-toolbar");
      var descriptionCharHint = document.getElementById("create-entity-description-char-hint");
      var ownerHandlesInput = document.getElementById("create-entity-owner-handles");
      var parentCommunityHandleInput = document.getElementById("create-entity-parent-handle");
      var handleState = { status: "unknown", token: 0 };
      var handleBadge = ensureHandleAvailabilityBadge(handleInput, "inline");
      var placeState = { status: "idle", token: 0 };
      var CREATE_ENTITY_DESC_MAX_CHARS = 4000;

      function showEntityStatus(msg, isError) {
        if (!status) return;
        var t = String(msg || "").trim();
        status.hidden = !t;
        status.textContent = t;
        status.classList.toggle("create-post-submit-status--error", !!isError && !!t);
        status.classList.toggle("create-post-submit-status--success", !isError && !!t);
        status.classList.toggle("handle-availability--taken", !!isError && !!t);
        status.classList.toggle("handle-availability--ready", !isError && !!t);
      }

      function showEntityHandleBadge(msg, mode) {
        setHandleAvailabilityBadge(handleBadge, msg || "", mode || "");
      }

      function ensureRailSlotForHandle(rawHandle) {
        var h = String(rawHandle || "").trim().replace(/^@+/, "");
        if (!h) return;
        var railList = document.querySelector(".app-rail-slots");
        if (!railList) return;
        var href = "/home/profile/" + encodeURIComponent(h);
        if (railList.querySelector('a.app-rail-slot--filled[href="' + href + '"]')) return;
        // Keep markup in sync with `snippets::avatar_not_found_html` + `rail::rail_slots_html` (no-avatar branch).
        var missingAvatarStack =
          '<span class="avatar-stack"><span class="app-avatar-not-found" aria-hidden="true"><svg viewBox="0 0 40 40" width="20" height="20" xmlns="http://www.w3.org/2000/svg"><circle cx="20" cy="20" r="19" fill="rgba(3,4,8,0.94)"/><path d="M20 28.8a1.35 1.35 0 1 1 0 2.7 1.35 1.35 0 0 1 0-2.7zm0-20.4c-3.55 0-6.1 2.05-6.1 5.25a1.25 1.25 0 0 0 2.5 0c0-1.68 1.36-2.75 3.6-2.75 2.31 0 3.72 1.02 3.72 2.62 0 1.11-.7 1.93-2.31 3.11-2.19 1.61-3.16 2.95-3.16 5.26v1.13a1.25 1.25 0 1 0 2.5 0v-1.13c0-1.36.48-2.07 2.14-3.29 2.05-1.5 3.33-2.82 3.33-5.08 0-3.15-2.66-5.12-6.22-5.12z" fill="var(--text)"/></svg></span></span>';
        var li = document.createElement("li");
        li.className = "app-rail-slot-item";
        li.setAttribute("role", "listitem");
        var a = document.createElement("a");
        a.className = "app-rail-slot app-rail-slot--filled";
        a.setAttribute("href", href);
        a.setAttribute("title", "@" + h);
        a.setAttribute("aria-label", "@" + h);
        var frame = document.createElement("span");
        frame.className = "brand-frame brand-frame--toolbar app-rail-slot-frame";
        frame.innerHTML = missingAvatarStack;
        a.appendChild(frame);
        li.appendChild(a);
        railList.appendChild(li);
      }

      function syncSubmitEnabled() {
        if (!submitBtn) return;
        var lid = locInput ? String(locInput.value || "").trim() : "";
        var handleReady = handleState.status === "available";
        if (isProfit || isNonProfit || isGathering) {
          submitBtn.disabled = !handleReady;
          return;
        }
        if (isCommunity) {
          var parentReady = handleHasMinLength(normalizeHandleCandidate(parentCommunityHandleInput ? parentCommunityHandleInput.value : ""));
          var countryReady = communityCountrySelect
            ? String(communityCountrySelect.value || "").trim().length === 2
            : false;
          submitBtn.disabled = !handleReady || !parentReady || !countryReady;
          return;
        }
        var placeReady = placeState.status === "verified";
        submitBtn.disabled = !handleReady || !placeReady || !lid || lid === "0";
      }

      function setPlaceBadge(msg, mode) {
        setHandleAvailabilityBadge(placeBadge, msg || "", mode || "");
      }

      function syncDescriptionField() {
        if (EntityFormCommon.syncRichDescription) {
          EntityFormCommon.syncRichDescription(descriptionInput, descriptionEditor);
        }
      }

      if (EntityFormCommon.bindRichDescription) {
        EntityFormCommon.bindRichDescription({
          descriptionInput: descriptionInput,
          descriptionEditor: descriptionEditor,
          descriptionToolbar: descriptionToolbar,
          descriptionCharHint: descriptionCharHint,
          maxChars: CREATE_ENTITY_DESC_MAX_CHARS,
        });
      }

      if (EntityFormCommon.bindOwnerHandles && ownerHandlesInput) {
        EntityFormCommon.bindOwnerHandles(ownerHandlesInput);
      }

      var communityCountrySelect = document.getElementById("create-entity-community-country");
      var communityRegionInput = document.getElementById("create-entity-community-region");
      var communityLocalityInput = document.getElementById("create-entity-community-locality");
      var communityLocaleFields = document.getElementById("create-entity-community-locale-fields");
      var communityLocationHint = document.getElementById("create-entity-community-location-hint");
      var communityJoinPolicySelect = document.getElementById("create-entity-community-join-policy");
      var communityJoinPolicyHint = document.getElementById("create-entity-community-join-policy-hint");
      var COMMUNITY_GLOBAL_COUNTRY = "ZZ";
      var communityHandleSuffixHint = document.getElementById("create-entity-community-handle-suffix-hint");
      var communitySeedClauseInput = document.getElementById("create-entity-community-seed-clause");
      var communitySeedClauseEditor = document.getElementById("create-entity-community-seed-clause-editor");
      var communitySeedClauseToolbar = form.querySelector(".create-entity-seed-clause-toolbar");
      var communitySeedClauseCharHint = document.getElementById("create-entity-community-seed-clause-char-hint");
      var parentCommunityResults = document.getElementById("create-entity-parent-results");
      var parentCommunitySearchStatus = document.getElementById("create-entity-parent-search-status");
      var parentConstitutionWrap = document.getElementById("create-entity-parent-constitution-wrap");
      var communityHandleAutoSuffix = true;
      var CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS = 2000;
      var parentConstitutionToken = 0;

      function slugLocationSegment(raw) {
        return String(raw || "")
          .trim()
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "_")
          .replace(/^_+|_+$/g, "")
          .replace(/_+/g, "_");
      }

      function isCommunityGlobalCountry(code) {
        return String(code || "")
          .trim()
          .toUpperCase() === COMMUNITY_GLOBAL_COUNTRY;
      }

      function communityHandleSuffixFromLocation() {
        var cc = communityCountrySelect
          ? String(communityCountrySelect.value || "").trim().toUpperCase()
          : "";
        if (!cc || cc.length !== 2 || isCommunityGlobalCountry(cc)) return "";
        cc = slugLocationSegment(cc);
        if (!cc) return "";
        var parts = [];
        var locality = communityLocalityInput
          ? slugLocationSegment(communityLocalityInput.value)
          : "";
        var region = communityRegionInput ? slugLocationSegment(communityRegionInput.value) : "";
        if (locality) parts.push(locality);
        if (region) parts.push(region);
        parts.push(cc);
        return parts.length ? "_" + parts.join("_") : "";
      }

      function fitCommunityHandle(base, suffix) {
        var maxLen = 32;
        suffix = String(suffix || "");
        base = String(base || "")
          .replace(/_+$/g, "")
          .replace(/^_+/g, "");
        if (!suffix) return base.slice(0, maxLen);
        if (!base) return suffix.replace(/^_+/, "").slice(0, maxLen);
        var combined = base + suffix;
        if (combined.length <= maxLen) return combined;
        var room = maxLen - suffix.length;
        if (room < 1) return suffix.slice(0, maxLen);
        return (base.slice(0, room) + suffix).slice(0, maxLen);
      }

      function splitCommunityHandleParts(full) {
        var suffix = communityHandleSuffixFromLocation();
        var handle = normalizeHandleCandidate(full);
        if (!suffix || !handle.endsWith(suffix.replace(/^_/, ""))) {
          return { base: handle, suffix: "" };
        }
        var bareSuffix = suffix.replace(/^_/, "");
        if (handle.length <= bareSuffix.length) {
          return { base: "", suffix: suffix };
        }
        var base = handle.slice(0, handle.length - bareSuffix.length);
        if (base.endsWith("_")) base = base.slice(0, -1);
        return { base: base, suffix: suffix };
      }

      function updateCommunityHandleSuffixUi() {
        if (!isCommunity) return;
        var suffix = communityHandleSuffixFromLocation();
        if (communityHandleSuffixHint) {
          if (suffix) {
            var template =
              UI.create_entity_community_handle_suffix_hint ||
              "Locale suffix: {suffix} (e.g. mums{suffix})";
            communityHandleSuffixHint.textContent = template.replace("{suffix}", suffix);
            communityHandleSuffixHint.hidden = false;
          } else {
            communityHandleSuffixHint.textContent = "";
            communityHandleSuffixHint.hidden = true;
          }
        }
        if (!communityHandleAutoSuffix || !handleInput || !suffix) return;
        var parts = splitCommunityHandleParts(handleInput.value);
        var next = fitCommunityHandle(parts.base, suffix);
        if (next && next !== normalizeHandleCandidate(handleInput.value)) {
          handleInput.value = next;
          checkCreateEntityHandleDebounced();
        }
      }

      if (isCommunity) {
        function updateJoinPolicyHint() {
          if (!communityJoinPolicyHint || !communityJoinPolicySelect) return;
          var key = String(communityJoinPolicySelect.value || "open").trim().toLowerCase();
          var hint =
            key === "closed"
              ? UI.create_entity_community_join_closed_hint
              : key === "reviewed"
                ? UI.create_entity_community_join_reviewed_hint
                : key === "invite_only"
                  ? UI.create_entity_community_join_invite_only_hint
                  : UI.create_entity_community_join_open_hint;
          communityJoinPolicyHint.textContent = hint || "";
        }

        function syncCommunityLocationUi() {
          var global = communityCountrySelect
            ? isCommunityGlobalCountry(communityCountrySelect.value)
            : true;
          if (communityLocaleFields) {
            communityLocaleFields.disabled = global;
          }
          if (global) {
            if (communityRegionInput) communityRegionInput.value = "";
            if (communityLocalityInput) communityLocalityInput.value = "";
          }
          if (communityLocationHint) {
            communityLocationHint.textContent = global
              ? UI.create_entity_community_location_global_hint ||
                "Global scope — like the seed council communities."
              : UI.create_entity_community_location_country_hint ||
                "Locked to this country. Add region and town or city for locale-encoded handles.";
          }
          updateCommunityHandleSuffixUi();
        }

        var onCommunityLocationChange = function () {
          syncCommunityLocationUi();
          syncSubmitEnabled();
        };

        if (communityCountrySelect) {
          var communityCountryPrevious = String(communityCountrySelect.value || COMMUNITY_GLOBAL_COUNTRY);
          communityCountrySelect.addEventListener("change", function () {
            var next = String(communityCountrySelect.value || "").trim().toUpperCase();
            var prev = String(communityCountryPrevious || "").trim().toUpperCase();
            if (next !== prev && (next === COMMUNITY_GLOBAL_COUNTRY || prev === COMMUNITY_GLOBAL_COUNTRY)) {
              var confirmMsg =
                next === COMMUNITY_GLOBAL_COUNTRY
                  ? UI.create_entity_community_confirm_global ||
                    "Switch to global scope? This community will not be tied to a country."
                  : UI.create_entity_community_confirm_country_lock ||
                    "Lock this community to the selected country?";
              if (!window.confirm(confirmMsg)) {
                communityCountrySelect.value = prev;
                return;
              }
            }
            communityCountryPrevious = String(communityCountrySelect.value || "");
            onCommunityLocationChange();
          });
          syncCommunityLocationUi();
        }
        if (communityRegionInput) {
          communityRegionInput.addEventListener("input", onCommunityLocationChange);
        }
        if (communityLocalityInput) {
          communityLocalityInput.addEventListener("input", onCommunityLocationChange);
        }
        if (communityJoinPolicySelect) {
          communityJoinPolicySelect.addEventListener("change", updateJoinPolicyHint);
          updateJoinPolicyHint();
        }
        if (handleInput) {
          handleInput.addEventListener("input", function () {
            if (handleInput.getAttribute("data-community-handle-auto-suffix") === "1") {
              var suffix = communityHandleSuffixFromLocation();
              var handle = normalizeHandleCandidate(handleInput.value);
              if (suffix && !handle.endsWith(suffix.replace(/^_/, ""))) {
                communityHandleAutoSuffix = false;
                handleInput.setAttribute("data-community-handle-auto-suffix", "0");
              }
            }
          });
        }
        function communitySeedClausePlain() {
          if (!communitySeedClauseEditor) return "";
          return String(communitySeedClauseEditor.innerText || "")
            .replace(/\r\n/g, "\n")
            .trim();
        }

        function syncCommunitySeedClauseField() {
          if (!communitySeedClauseInput) return;
          communitySeedClauseInput.value = communitySeedClausePlain();
        }

        function updateCommunitySeedClauseHint() {
          if (!communitySeedClauseCharHint) return;
          var used = Math.max(
            0,
            Math.min(CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS, communitySeedClausePlain().length)
          );
          communitySeedClauseCharHint.textContent =
            String(used) + "/" + String(CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS);
          communitySeedClauseCharHint.classList.toggle(
            "is-limit",
            used >= CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS
          );
        }

        function enforceCommunitySeedClauseLimit() {
          if (!communitySeedClauseEditor) return;
          if (communitySeedClausePlain().length <= CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS) return;
          try {
            document.execCommand("undo", false, null);
          } catch (_) {}
          if (communitySeedClausePlain().length > CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS) {
            communitySeedClauseEditor.textContent = communitySeedClausePlain().slice(
              0,
              CREATE_ENTITY_SEED_CLAUSE_MAX_CHARS
            );
          }
        }

        if (communitySeedClauseToolbar && communitySeedClauseEditor) {
          var seedColorBtns = communitySeedClauseToolbar.querySelectorAll(
            ".create-post-editor-color-btn"
          );
          seedColorBtns.forEach(function (btn) {
            btn.textContent = "";
            var swatch = (btn.getAttribute("data-value") || "").trim();
            if (swatch) btn.style.setProperty("--swatch", swatch);
          });
          communitySeedClauseToolbar.addEventListener("click", function (e) {
            var btn = e.target && e.target.closest ? e.target.closest("[data-cmd]") : null;
            if (!btn) return;
            e.preventDefault();
            var cmd = btn.getAttribute("data-cmd");
            if (!cmd) return;
            communitySeedClauseEditor.focus();
            var value = btn.getAttribute("data-value");
            document.execCommand(cmd, false, value != null ? value : null);
            enforceCommunitySeedClauseLimit();
            syncCommunitySeedClauseField();
            updateCommunitySeedClauseHint();
          });
          communitySeedClauseEditor.addEventListener("input", function () {
            enforceCommunitySeedClauseLimit();
            syncCommunitySeedClauseField();
            updateCommunitySeedClauseHint();
          });
          communitySeedClauseEditor.addEventListener("paste", function () {
            window.setTimeout(function () {
              enforceCommunitySeedClauseLimit();
              syncCommunitySeedClauseField();
              updateCommunitySeedClauseHint();
            }, 0);
          });
          syncCommunitySeedClauseField();
          updateCommunitySeedClauseHint();
        }

        function showParentSearchStatus(msg, isError) {
          if (!parentCommunitySearchStatus) return;
          var t = String(msg || "").trim();
          parentCommunitySearchStatus.hidden = !t;
          parentCommunitySearchStatus.textContent = t;
          parentCommunitySearchStatus.classList.toggle("create-post-submit-status--error", !!isError && !!t);
        }

        function clearParentCommunityResults() {
          if (parentCommunityResults) parentCommunityResults.innerHTML = "";
          if (parentCommunityHandleInput) {
            parentCommunityHandleInput.setAttribute("aria-expanded", "false");
          }
        }

        function renderParentCommunityResults(items) {
          if (!parentCommunityResults) return;
          if (!items || !items.length) {
            parentCommunityResults.innerHTML = "";
            if (parentCommunityHandleInput) {
              parentCommunityHandleInput.setAttribute("aria-expanded", "false");
            }
            return;
          }
          parentCommunityResults.innerHTML = items
            .map(function (it) {
              return renderSearchAddResultRow(it, {
                asButton: true,
                buttonClass: "create-entity-parent-pick-btn",
              });
            })
            .join("");
          if (parentCommunityHandleInput) {
            parentCommunityHandleInput.setAttribute("aria-expanded", "true");
          }
        }

        function bindConstitutionExpanderInRoot(root) {
          if (!root) return;
          var body = root.querySelector("#profile-constitution-body");
          var btn = root.querySelector("[data-profile-constitution-expand]");
          if (!body || !btn) return;
          body.classList.add("is-clamped");
          var showMore = UI.constitution_show_more || "Show more …";
          var showLess = UI.constitution_show_less || "Show less";
          function syncButton() {
            var hasOverflow = body.scrollHeight > body.clientHeight + 1;
            btn.hidden = !hasOverflow;
            if (!hasOverflow) {
              body.classList.remove("is-clamped");
              body.classList.remove("is-expanded");
              btn.setAttribute("aria-expanded", "false");
              btn.textContent = showMore;
              return;
            }
            if (!body.classList.contains("is-expanded")) {
              body.classList.add("is-clamped");
            }
          }
          btn.onclick = function () {
            var expanded = body.classList.toggle("is-expanded");
            body.classList.toggle("is-clamped", !expanded);
            btn.setAttribute("aria-expanded", expanded ? "true" : "false");
            btn.textContent = expanded ? showLess : showMore;
          };
          syncButton();
          if (typeof requestAnimationFrame === "function") {
            requestAnimationFrame(syncButton);
          }
        }

        function clearParentConstitutionPreview() {
          if (!parentConstitutionWrap) return;
          parentConstitutionWrap.innerHTML = "";
          parentConstitutionWrap.hidden = true;
        }

        function showParentConstitutionLoading() {
          if (!parentConstitutionWrap) return;
          parentConstitutionWrap.innerHTML =
            '<p class="hint">' +
            escapeHtml(UI.constitution_loading || "Loading constitution…") +
            "</p>";
          parentConstitutionWrap.hidden = false;
        }

        function loadParentConstitutionPreview(handle) {
          if (!parentConstitutionWrap) return;
          var normalized = normalizeHandleCandidate(handle);
          if (!handleHasMinLength(normalized)) {
            clearParentConstitutionPreview();
            return;
          }
          var token = parentConstitutionToken + 1;
          parentConstitutionToken = token;
          showParentConstitutionLoading();
          fetch(
            EFFECTIVE_CONSTITUTION_URL + "?handle=" + encodeURIComponent(normalized),
            { credentials: "same-origin", headers: { Accept: "application/json" } }
          )
            .then(function (r) {
              if (!r.ok) throw new Error("constitution_load_failed");
              return r.json();
            })
            .then(function (data) {
              if (token !== parentConstitutionToken) return;
              if (!data || !data.html) {
                clearParentConstitutionPreview();
                return;
              }
              parentConstitutionWrap.innerHTML = data.html;
              parentConstitutionWrap.removeAttribute("hidden");
              parentConstitutionWrap.hidden = false;
              bindConstitutionExpanderInRoot(parentConstitutionWrap);
            })
            .catch(function () {
              if (token !== parentConstitutionToken) return;
              clearParentConstitutionPreview();
              showParentSearchStatus(
                UI.constitution_load_failed || "Could not load inherited constitution.",
                true
              );
            });
        }

        var loadParentConstitutionDebounced = debounce(function () {
          if (!parentCommunityHandleInput) return;
          loadParentConstitutionPreview(parentCommunityHandleInput.value);
        }, 280);

        var runParentCommunitySearch = debounce(function () {
          var q = parentCommunityHandleInput
            ? String(parentCommunityHandleInput.value || "").trim().slice(0, 32)
            : "";
          if (parentCommunityHandleInput && parentCommunityHandleInput.value.length > 32) {
            parentCommunityHandleInput.value = q;
          }
          if (!shouldTriggerSearch(q)) {
            clearParentCommunityResults();
            showParentSearchStatus("", false);
            return;
          }
          showParentSearchStatus("", false);
          fetch(
            SEARCH_ENTITIES_URL +
              "?q=" +
              encodeURIComponent(q) +
              "&type_key=community",
            { credentials: "same-origin", headers: { Accept: "application/json" } }
          )
            .then(function (r) {
              if (r.status === 429) {
                return r.json().then(function (d) {
                  var err = new Error("rate_limited");
                  err.rateMsg = (d && d.error) || UI.card_rate_limited;
                  throw err;
                });
              }
              if (!r.ok) throw new Error("search_failed");
              return r.json();
            })
            .then(function (items) {
              renderParentCommunityResults(Array.isArray(items) ? items : []);
            })
            .catch(function (e) {
              clearParentCommunityResults();
              showParentSearchStatus(
                (e && e.rateMsg) || UI.search_add_search_failed || "Search failed.",
                true
              );
            });
        }, 220);

        function shouldTriggerSearch(raw) {
          var query = String(raw || "").trim().slice(0, 32);
          if (!query) return false;
          if (query.charAt(0) === "@") {
            var body = query.slice(1);
            var alnum = (body.match(/[A-Za-z0-9]/g) || []).length;
            return alnum >= 3;
          }
          return query.length >= 3;
        }

        function pickParentCommunity(handle) {
          if (!parentCommunityHandleInput) return;
          var normalized = normalizeHandleCandidate(handle);
          parentCommunityHandleInput.value = normalized ? "@" + normalized : "";
          clearParentCommunityResults();
          showParentSearchStatus("", false);
          syncSubmitEnabled();
          loadParentConstitutionPreview(normalized);
        }

        if (parentCommunityHandleInput) {
          parentCommunityHandleInput.addEventListener("input", function () {
            runParentCommunitySearch();
            loadParentConstitutionDebounced();
            syncSubmitEnabled();
          });
          parentCommunityHandleInput.addEventListener("blur", function () {
            window.setTimeout(function () {
              clearParentCommunityResults();
            }, 180);
            loadParentConstitutionPreview(parentCommunityHandleInput.value);
            syncSubmitEnabled();
          });
        }

        if (parentCommunityResults) {
          parentCommunityResults.addEventListener("click", function (e) {
            var btn =
              e.target && e.target.closest
                ? e.target.closest(".create-entity-parent-pick-btn")
                : null;
            if (!btn) return;
            e.preventDefault();
            pickParentCommunity(btn.getAttribute("data-handle") || "");
          });
        }

        if (parentCommunityHandleInput) {
          var initialParent = normalizeHandleCandidate(parentCommunityHandleInput.value);
          if (handleHasMinLength(initialParent)) {
            loadParentConstitutionPreview(initialParent);
          }
        }
      }

      var lidInit = locInput ? String(locInput.value || "").trim() : "";
      if (!isProfit && !isNonProfit && !isGathering && !isCommunity && (!lidInit || lidInit === "0")) {
        if (warn) warn.hidden = false;
      }
      if (isProfit || isNonProfit || isGathering || isCommunity) {
        syncSubmitEnabled();
      } else if (!lidInit || lidInit === "0") {
        if (submitBtn) submitBtn.disabled = true;
      }

      var checkCreateEntityHandleDebounced = debounce(function () {
        var normalized = normalizeHandleCandidate(handleInput ? handleInput.value : "");
        if (!handleHasMinLength(normalized)) {
          handleState.status = "too_short";
          showEntityStatus("", false);
          showEntityHandleBadge(UI.handle_min_chars || UI.create_entity_handle_hint || "", "error");
          syncSubmitEnabled();
          return;
        }
        handleState.status = "checking";
        showEntityStatus("", false);
        showEntityHandleBadge(UI.handle_checking || "Checking handle...", "checking");
        syncSubmitEnabled();
        var token = handleState.token + 1;
        handleState.token = token;
        fetch("/social-entities/handle-availability?handle=" + encodeURIComponent(normalized), {
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        })
          .then(function (r) {
            if (!r.ok) throw new Error("handle_check_failed");
            return r.json();
          })
          .then(function (data) {
            if (handleState.token !== token) return;
            var hStatus = String((data && data.status) || "");
            handleState.status = hStatus;
            if (hStatus === "available") {
              showEntityStatus("", false);
              showEntityHandleBadge(UI.handle_ready || "Ready to Go", "success");
            } else if (hStatus === "taken") {
              showEntityStatus("", false);
              showEntityHandleBadge(UI.handle_taken || "Already taken", "error");
            } else if (hStatus === "reserved") {
              showEntityStatus("", false);
              showEntityHandleBadge(UI.handle_reserved || "This handle is reserved", "error");
            } else if (hStatus === "invalid") {
              showEntityStatus("", false);
              showEntityHandleBadge(UI.handle_invalid || UI.create_entity_handle_hint || "", "error");
            } else if (hStatus === "too_short") {
              showEntityStatus("", false);
              showEntityHandleBadge(UI.handle_min_chars || UI.create_entity_handle_hint || "", "error");
            } else {
              showEntityStatus("", false);
              showEntityHandleBadge("", "");
            }
            syncSubmitEnabled();
          })
          .catch(function () {
            if (handleState.token !== token) return;
            handleState.status = "error";
            showEntityStatus("", false);
            showEntityHandleBadge(UI.handle_check_failed || UI.request_failed || "Handle check failed.", "error");
            syncSubmitEnabled();
          });
      }, 220);

      if (handleInput) {
        applyHandleInputDefaults(handleInput, { keepPrefixOnFocus: true, useDefaultPlaceholder: true });
        handleInput.addEventListener("input", checkCreateEntityHandleDebounced);
        checkCreateEntityHandleDebounced();
        if (isCommunity) updateCommunityHandleSuffixUi();
      }
      if (isCommunity && parentCommunityHandleInput) {
        applyHandleInputDefaults(parentCommunityHandleInput, {
          keepPrefixOnFocus: true,
          useDefaultPlaceholder: true,
        });
      }

      if ((isProfit || isNonProfit) && placeIdInput && EntityFormCommon.bindGooglePlaceSearch) {
        EntityFormCommon.bindGooglePlaceSearch({
          locInput: locInput,
          placeQueryInput: placeQueryInput,
          placeResults: placeResults,
          placeBadge: placeBadge,
          placeIdInput: placeIdInput,
          UI: UI,
          escapeHtml: escapeHtml,
          buildHeaders: mutationHeaders,
          clearLocationOnQueryChange: true,
          debounceMs: 250,
          onVerified: function () {
            placeState.status = "verified";
            syncSubmitEnabled();
          },
          onVerifyError: function () {
            placeState.status = "error";
            syncSubmitEnabled();
          },
          onQueryChange: function () {
            placeState.status = "idle";
            syncSubmitEnabled();
          },
        });
      }

      if (backBtn) {
        backBtn.addEventListener("click", function () {
          if (!slot) return;
          reloadSearchAddPanel(slot)
            .then(function () {
              scrollFeedToTop(false);
            })
            .catch(function (err) {
              enqueueRightPanelNotification(
                (err && err.rateMsg) || UI.search_add_load_failed || "Could not reload search/add.",
                3000
              );
            });
        });
      }

      form.addEventListener("submit", function (ev) {
        ev.preventDefault();
        var typeIdEl = document.getElementById("create-entity-type-id");
        var typeId = typeIdEl ? String(typeIdEl.value || "").trim() : "";
        var locationId = locInput ? String(locInput.value || "").trim() : "";
        var handle = normalizeHandleCandidate(handleInput ? handleInput.value : "");
        if (handleState.status !== "available") {
          showEntityStatus("", false);
          showEntityHandleBadge(UI.handle_taken || "Already taken", "error");
          return;
        }
        if (isCommunity) {
          var communityCountry = communityCountrySelect
            ? String(communityCountrySelect.value || "").trim().toUpperCase()
            : "";
          if (communityCountry.length !== 2) {
            showEntityStatus(
              UI.create_entity_community_location_required ||
                UI.create_entity_location_required ||
                "Choose a country for this community.",
              true
            );
            return;
          }
        } else if (!isProfit && !isNonProfit && !isGathering && (!locationId || locationId === "0")) {
          showEntityStatus(
            isProfit
              ? UI.create_entity_business_location_required ||
                  "Confirm a business address before creating."
              : UI.create_entity_location_required || "Location required.",
            true
          );
          return;
        }
        if (!typeId || !handle) return;

        var payload;
        if (isProfit) {
          var tradingEl = document.getElementById("create-entity-trading-label");
          var legalEl = document.getElementById("create-entity-legal-name");
          var foundedEl = document.getElementById("create-entity-founded");
          var intentionEl = document.getElementById("create-entity-intention");
          syncDescriptionField();
          var label = tradingEl ? String(tradingEl.value || "").trim() : "";
          var legalName = legalEl ? String(legalEl.value || "").trim() : "";
          if (!label || !legalName) return;
          var foundedRaw = foundedEl ? String(foundedEl.value || "").trim() : "";
          if (!foundedRaw) {
            showEntityStatus(UI.create_entity_founded_required || UI.request_failed || "Founded date is required.", true);
            return;
          }
          var intention = intentionEl ? String(intentionEl.value || "").trim() : "";
          var description = descriptionInput ? String(descriptionInput.value || "").trim() : "";
          var websiteEl = document.getElementById("create-entity-website");
          var logoUrlEl = document.getElementById("create-entity-logo-url");
          var ownersEl = ownerHandlesInput;
          var websiteUrl = websiteEl ? String(websiteEl.value || "").trim() : "";
          var logoUrl = logoUrlEl ? String(logoUrlEl.value || "").trim() : "";
          var ownerHandles = ownersEl
            ? (EntityFormCommon.stripOwnerHandlesPrefix
                ? EntityFormCommon.stripOwnerHandlesPrefix(ownersEl.value)
                : String(ownersEl.value || "").trim())
            : "";
          payload = {
            handle: handle,
            type_id: typeId,
            location_id: locationId,
            profit: {
              label: label,
              legal_name: legalName,
              founded_at: foundedRaw || null,
              intention: intention || null,
              description: description || null,
              website_url: websiteUrl || null,
              logo_url: logoUrl || null,
              owner_handles: ownerHandles || null,
            },
          };
        } else {
          payload = {
            handle: handle,
            type_id: typeId,
            location_id: locationId,
          };
          if (isNonProfit) {
            var tradingElNp = document.getElementById("create-entity-trading-label");
            var legalElNp = document.getElementById("create-entity-legal-name");
            var foundedElNp = document.getElementById("create-entity-founded");
            var intentionElNp = document.getElementById("create-entity-intention");
            syncDescriptionField();
            var labelNp = tradingElNp ? String(tradingElNp.value || "").trim() : "";
            var legalNameNp = legalElNp ? String(legalElNp.value || "").trim() : "";
            if (!labelNp || !legalNameNp) return;
            var foundedRawNp = foundedElNp ? String(foundedElNp.value || "").trim() : "";
            if (!foundedRawNp) {
              showEntityStatus(UI.create_entity_founded_required || UI.request_failed || "Founded date is required.", true);
              return;
            }
            var websiteElNp = document.getElementById("create-entity-website");
            var logoUrlElNp = document.getElementById("create-entity-logo-url");
            var ownersElNp = ownerHandlesInput;
            var descriptionNp = descriptionInput ? String(descriptionInput.value || "").trim() : "";
            var intentionNp = intentionElNp ? String(intentionElNp.value || "").trim() : "";
            var websiteUrlNp = websiteElNp ? String(websiteElNp.value || "").trim() : "";
            var logoUrlNp = logoUrlElNp ? String(logoUrlElNp.value || "").trim() : "";
            var ownerHandlesNp = ownersElNp
              ? (EntityFormCommon.stripOwnerHandlesPrefix
                  ? EntityFormCommon.stripOwnerHandlesPrefix(ownersElNp.value)
                  : String(ownersElNp.value || "").trim())
              : "";
            payload.non_profit = {
              label: labelNp,
              legal_name: legalNameNp,
              founded_at: foundedRawNp || null,
              intention: intentionNp || null,
              description: descriptionNp || null,
              website_url: websiteUrlNp || null,
              logo_url: logoUrlNp || null,
              owner_handles: ownerHandlesNp || null,
            };
          } else if (isGathering) {
            var gatheringLabelEl = document.getElementById("create-entity-trading-label");
            var gatheringIntentionEl = document.getElementById("create-entity-intention");
            var gatheringEventUrlEl = document.getElementById("create-entity-event-url");
            syncDescriptionField();
            var gatheringLabel = gatheringLabelEl ? String(gatheringLabelEl.value || "").trim() : "";
            if (!gatheringLabel) return;
            var gatheringDescription = descriptionInput ? String(descriptionInput.value || "").trim() : "";
            var gatheringIntention = gatheringIntentionEl ? String(gatheringIntentionEl.value || "").trim() : "";
            var gatheringEventUrl = gatheringEventUrlEl ? String(gatheringEventUrlEl.value || "").trim() : "";
            var gatheringOwnerHandles = ownerHandlesInput
              ? (EntityFormCommon.stripOwnerHandlesPrefix
                  ? EntityFormCommon.stripOwnerHandlesPrefix(ownerHandlesInput.value)
                  : String(ownerHandlesInput.value || "").trim())
              : "";
            payload.gathering = {
              label: gatheringLabel,
              intention: gatheringIntention || null,
              description: gatheringDescription || null,
              event_url: gatheringEventUrl || null,
              owner_handles: gatheringOwnerHandles || null,
            };
          } else if (isCommunity) {
            var communityLabelEl = document.getElementById("create-entity-trading-label");
            var communityIntentionEl = document.getElementById("create-entity-intention");
            var communityParentHandleEl = parentCommunityHandleInput;
            syncDescriptionField();
            var communityLabel = communityLabelEl ? String(communityLabelEl.value || "").trim() : "";
            if (!communityLabel) return;
            var parentCommunityHandle = normalizeHandleCandidate(
              communityParentHandleEl ? communityParentHandleEl.value : ""
            );
            if (!handleHasMinLength(parentCommunityHandle)) {
              showEntityStatus(UI.create_entity_community_parent_required || UI.request_failed || "Parent community is required.", true);
              return;
            }
            var communityDescription = descriptionInput ? String(descriptionInput.value || "").trim() : "";
            var communityIntention = communityIntentionEl ? String(communityIntentionEl.value || "").trim() : "";
            var communityOwnerHandles = ownerHandlesInput
              ? (EntityFormCommon.stripOwnerHandlesPrefix
                  ? EntityFormCommon.stripOwnerHandlesPrefix(ownerHandlesInput.value)
                  : String(ownerHandlesInput.value || "").trim())
              : "";
            var joinPolicyEl = document.getElementById("create-entity-community-join-policy");
            var joinPolicyKey = joinPolicyEl ? String(joinPolicyEl.value || "").trim().toLowerCase() : "open";
            if (
              joinPolicyKey !== "open" &&
              joinPolicyKey !== "reviewed" &&
              joinPolicyKey !== "invite_only" &&
              joinPolicyKey !== "closed"
            ) {
              joinPolicyKey = "open";
            }
            var communityCountryCode = communityCountrySelect
              ? String(communityCountrySelect.value || "").trim().toUpperCase()
              : "";
            if (communityCountryCode.length !== 2) {
              showEntityStatus(
                UI.create_entity_community_location_required ||
                  UI.create_entity_location_required ||
                  "Choose global or a country for this community.",
                true
              );
              return;
            }
            var communityRegion = isCommunityGlobalCountry(communityCountryCode)
              ? ""
              : communityRegionInput
                ? String(communityRegionInput.value || "").trim()
                : "";
            var communityLocality = isCommunityGlobalCountry(communityCountryCode)
              ? ""
              : communityLocalityInput
                ? String(communityLocalityInput.value || "").trim()
                : "";
            if (communitySeedClauseEditor && communitySeedClauseInput) {
              communitySeedClauseInput.value = String(communitySeedClauseEditor.innerText || "")
                .replace(/\r\n/g, "\n")
                .trim();
            }
            var seedClause = communitySeedClauseInput
              ? String(communitySeedClauseInput.value || "").trim()
              : "";
            payload.location_id = "";
            payload.community = {
              label: communityLabel,
              parent_community_handle: parentCommunityHandle,
              location: {
                country_code: communityCountryCode,
                region: communityRegion || null,
                locality: communityLocality || null,
              },
              join_policy_key: joinPolicyKey,
              intention: communityIntention || null,
              description: communityDescription || null,
              owner_handles: communityOwnerHandles || null,
              seed_constitution_clause: seedClause || null,
            };
          }
        }

        if (submitBtn) {
          submitBtn.disabled = true;
          submitBtn.classList.add("is-loading");
        }
        showEntityStatus("", false);
        fetch("/social-entities", {
          method: "POST",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify(payload),
        })
          .then(function (r) {
            return r
              .json()
              .catch(function () {
                return {};
              })
              .then(function (d) {
                if (!r.ok) {
                  var msg = (d && d.error) || "Create failed.";
                  throw new Error(msg);
                }
                enqueueRightPanelNotification(
                  (UI.search_add_create_success || "Created: ") +
                    "@" +
                    String(d.handle || handle).replace(/^@+/, ""),
                  3000
                );
                ensureRailSlotForHandle(String(d.handle || handle));
                if (slot) {
                  return reloadSearchAddPanel(slot).then(function () {
                    var searchInput = getRailSearchInput();
                    if (searchInput) {
                      setRailSearchActive(true);
                      searchInput.value = "@" + String(d.handle || handle).replace(/^@+/, "");
                      var evInput;
                      try {
                        evInput = new Event("input", { bubbles: true });
                      } catch (_) {
                        evInput = document.createEvent("Event");
                        evInput.initEvent("input", true, true);
                      }
                      searchInput.dispatchEvent(evInput);
                    }
                  });
                }
              });
          })
          .catch(function (err) {
            showEntityStatus((err && err.message) || UI.request_failed || "Request failed.", true);
          })
          .then(function () {
            if (submitBtn) {
              submitBtn.disabled = false;
              submitBtn.classList.remove("is-loading");
            }
            if (isProfit || isNonProfit || isGathering || isCommunity) {
              syncSubmitEnabled();
            }
          });
      });
    }

    function updateFeedNavButtons(meta) {
      var hasNewer = currentFeedPage > 0;
      var hasOlder = !!(meta && meta.has_older);
      var subnav = document.getElementById("app-subnav");
      if (subnav) {
        if (hasNewer) subnav.setAttribute("data-has-newer-pages", "1");
        else subnav.removeAttribute("data-has-newer-pages");
      }
      var firstBtn = document.querySelector('[data-feed-nav="first"]');
      var prevBtn = document.querySelector('[data-feed-nav="prev"]');
      var nextBtn = document.querySelector('[data-feed-nav="next"]');
      if (firstBtn) {
        firstBtn.hidden = !hasNewer;
        firstBtn.disabled = !hasNewer;
        firstBtn.setAttribute("aria-disabled", hasNewer ? "false" : "true");
        var firstItem = firstBtn.closest(".app-rail-slot-item--feed-nav");
        if (firstItem) firstItem.hidden = !hasNewer;
      }
      if (prevBtn) {
        prevBtn.hidden = !hasNewer;
        prevBtn.disabled = !hasNewer;
        prevBtn.setAttribute("aria-disabled", hasNewer ? "false" : "true");
        var prevItem = prevBtn.closest(".app-rail-slot-item--feed-nav");
        if (prevItem) prevItem.hidden = !hasNewer;
      }
      if (nextBtn) {
        nextBtn.hidden = !hasOlder;
        nextBtn.disabled = !hasOlder;
        nextBtn.setAttribute("aria-disabled", hasOlder ? "false" : "true");
      }
      syncFeedScrollTopButtonOffset();
    }

    function handleFeedNavClick(e) {
      var btn = e.target && e.target.closest ? e.target.closest("[data-feed-nav]") : null;
      if (!btn || btn.disabled || btn.getAttribute("aria-disabled") === "true") return;
      e.preventDefault();
      var nav = (btn.getAttribute("data-feed-nav") || "").trim();
      if (nav === "first") {
        pendingFeedScrollToTop = true;
        loadFeedDay(0);
        return;
      }
      if (nav === "prev") {
        pendingFeedScrollToTop = true;
        loadFeedDay(clampFeedPage(currentFeedPage - 1));
        return;
      }
      if (nav === "next") {
        pendingFeedScrollToTop = true;
        loadFeedDay(clampFeedPage(currentFeedPage + 1));
        return;
      }
    }

    function bindFeedControls() {
      var subnav = document.getElementById("app-subnav");
      if (subnav) {
        if (subnav.getAttribute("data-feed-controls-bound") === "1") {
          subnav.removeEventListener("click", handleFeedNavClick);
        }
        subnav.setAttribute("data-feed-controls-bound", "1");
        subnav.addEventListener("click", handleFeedNavClick);
      }
      updateFeedNavButtons(null);
    }

    function renderFeedShell() {
      if (!feedPanel) return;
      feedPanel.innerHTML =
        '<section class="app-feed-component">' +
        '<div id="app-feed-cards-wrap" class="app-feed-cards-wrap">' +
        defaultFeedHtml() +
        "</div>" +
        "</section>";
      ensureRightPanelNotificationHost();
      ensureFeedScrollTopButton();
      syncFeedScrollTopButton();
      ensureOgPreviewRevealBinding();
      ensureFeedCardsMasonryObserver();
      scheduleFeedCardsMasonry();
      bindFeedControls();
    }

    var rightPanelNotifyQueue = [];
    var rightPanelNotifyBusy = false;
    var activeView = "";
    var activeViewNonce = 0;
    var currentProfileHandle = "";
    var feedLoadGeneration = 0;
    var pendingCreatePostPreset = null;

    function hydrateFeedCards(cardsWrap) {
      if (!cardsWrap) return;
      bindFeedExpanders(cardsWrap);
      bindFeedJudicialWithdrawForms(cardsWrap);
      hydrateFeedCountryTooltips(cardsWrap);
      if (typeof window.bindPostEngagementFeed === "function") {
        window.bindPostEngagementFeed(cardsWrap);
      }
      hydrateFeedOgFallback(cardsWrap);
      ensureFeedCardsMasonryObserver();
      scheduleFeedCardsMasonry();
    }

    function beginView(viewName) {
      activeView = String(viewName || "");
      activeViewNonce += 1;
      return activeViewNonce;
    }

    function isCurrentView(viewName, nonce) {
      return activeView === String(viewName || "") && nonce === activeViewNonce;
    }

    function isFeedPanelViewNonce(nonce) {
      return (
        isCurrentView("feed", nonce) ||
        isCurrentView("feed-create", nonce) ||
        isCurrentView("feed-search-add", nonce) ||
        isCurrentView("private-profile", nonce)
      );
    }

    function ensureRightPanelNotificationHost() {
      var host = document.getElementById("app-global-notify");
      if (host) return host;
      host = document.createElement("div");
      host.id = "app-global-notify";
      host.className = "app-global-notify";
      document.body.appendChild(host);
      return host;
    }

    function drainRightPanelNotifications() {
      if (rightPanelNotifyBusy) return;
      if (!rightPanelNotifyQueue.length) return;
      var host = ensureRightPanelNotificationHost();
      if (!host) return;
      rightPanelNotifyBusy = true;
      var item = rightPanelNotifyQueue.shift();
      var toast = document.createElement("div");
      toast.className = "app-right-panel-toast";
      toast.setAttribute("role", "status");
      toast.setAttribute("aria-live", "polite");
      if (item.html || String(item.message).indexOf("nom-amount") >= 0) {
        toast.innerHTML = item.message;
      } else {
        toast.textContent = item.message;
      }
      host.appendChild(toast);
      var visibleMs = Math.max(300, Number(item.visibleMs) || 3000);
      window.setTimeout(function () {
        toast.classList.add("is-leaving");
        window.setTimeout(function () {
          try {
            toast.remove();
          } catch (_) {}
          rightPanelNotifyBusy = false;
          drainRightPanelNotifications();
        }, 240);
      }, visibleMs);
    }

    function enqueueRightPanelNotification(message, visibleMs) {
      if (!message) return;
      var html =
        String(message).indexOf("nom-amount") >= 0 ||
        String(message).indexOf("nom-icon") >= 0;
      rightPanelNotifyQueue.push({
        message: String(message),
        visibleMs: visibleMs || 3000,
        html: html,
      });
      drainRightPanelNotifications();
    }

    function showInactivityRefundNoticeFromQuery() {
      var params = new URLSearchParams(window.location.search || "");
      if (params.get("notice") !== "inactivity_standing_restored") return;
      var raw = (params.get("standing_restored") || "").trim();
      var amount = Number(raw);
      if (!raw || !Number.isFinite(amount) || amount <= 0) return;
      var tpl =
        UI.inactivity_standing_restored_notice ||
        "Welcome back — {amount} civic standing restored from your time away. The inactivity clock has reset.";
      var msg = tpl.replace("{amount}", amount.toFixed(2));
      ensureRightPanelNotificationHost();
      enqueueRightPanelNotification(msg, 5000);
      params.delete("notice");
      params.delete("standing_restored");
      var q = params.toString();
      var clean =
        window.location.pathname + (q ? "?" + q : "") + (window.location.hash || "");
      if (window.history && window.history.replaceState) {
        window.history.replaceState(window.history.state || {}, "", clean);
      }
    }

    function setFeedControlsLoading(isLoading) {
      var loadingEl = document.getElementById("app-feed-controls-loading");
      if (!loadingEl) return;
      if (
        isLoading &&
        window.brandCardLoader &&
        typeof window.brandCardLoader.feedControlsLoaderHtml === "function" &&
        !loadingEl.childElementCount
      ) {
        loadingEl.innerHTML = window.brandCardLoader.feedControlsLoaderHtml();
      }
      loadingEl.hidden = !isLoading;
      loadingEl.classList.toggle("is-loading", !!isLoading);
    }

    function setFeedCardsLoading(isLoading) {
      var cardsWrap = document.getElementById("app-feed-cards-wrap");
      if (!cardsWrap) return;
      cardsWrap.classList.toggle("is-loading", !!isLoading);
      if (!isLoading) scheduleFeedCardsMasonry();
    }

    var feedMasonryFrame = 0;
    var feedMasonryWrapObserver = null;
    var feedMasonryCardObserver = null;

    function cssLengthToPx(fromEl, raw, fallbackPx) {
      var value = String(raw || "").trim();
      if (!value) return fallbackPx;
      if (value.endsWith("px")) return parseFloat(value) || fallbackPx;
      var base =
        fromEl && fromEl.nodeType === 1
          ? parseFloat(getComputedStyle(fromEl).fontSize) || 16
          : parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      if (value.endsWith("rem")) return (parseFloat(value) || 0) * base;
      if (value.endsWith("em")) return (parseFloat(value) || 0) * base;
      var n = parseFloat(value);
      return isFinite(n) ? n : fallbackPx;
    }

    function feedCardsMasonryChildren(wrap) {
      return Array.prototype.filter.call(wrap.children, function (el) {
        return (
          el.nodeType === 1 &&
          (el.classList.contains("card") || el.classList.contains("public-post-card"))
        );
      });
    }

    function layoutFeedCardsMasonry() {
      var wrap = document.getElementById("app-feed-cards-wrap");
      if (!wrap || wrap.classList.contains("is-loading")) return;

      var rootStyle = getComputedStyle(document.documentElement);
      var wrapStyle = getComputedStyle(wrap);
      var cardMax = cssLengthToPx(
        document.documentElement,
        rootStyle.getPropertyValue("--app-right-panel-card-max"),
        680
      );
      var gap = cssLengthToPx(
        wrap,
        wrapStyle.gap || wrapStyle.rowGap || rootStyle.getPropertyValue("--app-feed-grid-gap"),
        18
      );
      var cards = feedCardsMasonryChildren(wrap);
      if (!cards.length) {
        wrap.classList.remove("is-masonry");
        wrap.style.height = "";
        cards.forEach(function (card) {
          card.style.position = "";
          card.style.left = "";
          card.style.top = "";
          card.style.width = "";
        });
        return;
      }

      var containerWidth = wrap.clientWidth;
      if (containerWidth <= 0) {
        wrap.classList.remove("is-masonry");
        wrap.style.height = "";
        cards.forEach(function (card) {
          card.style.position = "";
          card.style.left = "";
          card.style.top = "";
          card.style.width = "";
        });
        return;
      }

      var cols = Math.max(1, Math.floor((containerWidth + gap) / (cardMax + gap)));
      var colWidth = cols === 1 ? Math.min(containerWidth, cardMax) : cardMax;
      var gridWidth = cols * colWidth + (cols - 1) * gap;
      var offsetX = Math.max(0, (containerWidth - gridWidth) / 2);

      wrap.classList.add("is-masonry");

      cards.forEach(function (card) {
        card.style.position = "relative";
        card.style.left = "";
        card.style.top = "";
        card.style.width = colWidth + "px";
      });

      var colHeights = new Array(cols);
      for (var c = 0; c < cols; c++) colHeights[c] = 0;

      cards.forEach(function (card) {
        var minCol = 0;
        for (var i = 1; i < cols; i++) {
          if (colHeights[i] < colHeights[minCol]) minCol = i;
        }
        var x = offsetX + minCol * (colWidth + gap);
        var y = colHeights[minCol];
        card.style.position = "absolute";
        card.style.left = x + "px";
        card.style.top = y + "px";
        card.style.width = colWidth + "px";
        colHeights[minCol] += card.offsetHeight + gap;
      });

      var maxHeight = 0;
      for (var j = 0; j < cols; j++) {
        if (colHeights[j] > maxHeight) maxHeight = colHeights[j];
      }
      wrap.style.height = Math.max(0, maxHeight - gap) + "px";
      observeFeedCardsForMasonry(cards);
    }

    function scheduleFeedCardsMasonry() {
      if (typeof requestAnimationFrame !== "function") {
        layoutFeedCardsMasonry();
        return;
      }
      if (feedMasonryFrame) cancelAnimationFrame(feedMasonryFrame);
      feedMasonryFrame = requestAnimationFrame(function () {
        feedMasonryFrame = 0;
        layoutFeedCardsMasonry();
      });
    }

    function observeFeedCardsForMasonry(cards) {
      if (typeof ResizeObserver !== "function" || !cards || !cards.length) return;
      if (!feedMasonryCardObserver) {
        feedMasonryCardObserver = new ResizeObserver(function () {
          scheduleFeedCardsMasonry();
        });
      }
      feedMasonryCardObserver.disconnect();
      cards.forEach(function (card) {
        feedMasonryCardObserver.observe(card);
      });
    }

    function ensureFeedCardsMasonryObserver() {
      var wrap = document.getElementById("app-feed-cards-wrap");
      if (!wrap) return;
      if (typeof ResizeObserver === "function" && !feedMasonryWrapObserver) {
        feedMasonryWrapObserver = new ResizeObserver(function () {
          scheduleFeedCardsMasonry();
        });
        feedMasonryWrapObserver.observe(wrap);
      }
      if (!wrap._feedMasonryImgLoadBound) {
        wrap._feedMasonryImgLoadBound = true;
        wrap.addEventListener(
          "load",
          function (e) {
            if (e.target && e.target.tagName === "IMG") scheduleFeedCardsMasonry();
          },
          true
        );
      }
    }

    function isPlatformSelfLink(url) {
      var u = String(url || "").trim();
      if (!u) return false;
      try {
        var parsed = new URL(u, window.location.origin);
        return parsed.host === window.location.host;
      } catch (_) {
        return false;
      }
    }

    function firstUrlFromFeedCard(card) {
      var u = (card.getAttribute("data-link-url") || "").trim();
      if (u) return u;
      var body = card.querySelector(".public-post-body");
      if (!body) return "";
      var a = body.querySelector("a[href^='http://'], a[href^='https://']");
      if (a) return (a.getAttribute("href") || "").trim();
      var t = body.textContent || "";
      var m = t.match(/https?:\/\/[^\s<>"')\]]+/);
      return m ? m[0].replace(/[.,;:!?)]+$/, "") : "";
    }

    function feedOgLoadingHtml() {
      return window.brandCardLoader ? window.brandCardLoader.publicOgLoadingHtml() : "";
    }

    function panelCardLoaderHtml() {
      return window.brandCardLoader ? window.brandCardLoader.panelCardLoaderHtml() : "";
    }

    function feedCardOgAnchor(card) {
      var head = card.querySelector(".public-post-head");
      if (head) return head;
      return card.querySelector(".public-post-body");
    }

    function insertFeedOgFragment(card, html) {
      var anchor = feedCardOgAnchor(card);
      if (!anchor) return;
      anchor.insertAdjacentHTML("afterend", html);
    }

    var ogPreviewRevealBound = false;

    function ogPreviewDefaults() {
      return {
        revealLabel: (UI && UI.og_preview_reveal) || "Show preview",
        withheldHint: (UI && UI.og_preview_withheld_hint) || "",
      };
    }

    function ensureOgPreviewRevealBinding() {
      if (ogPreviewRevealBound) return;
      ogPreviewRevealBound = true;
      if (window.OgPreviewClient && window.OgPreviewClient.bindRevealClick) {
        window.OgPreviewClient.bindRevealClick(document, escapeHtml);
      }
    }

    function hydrateFeedOgFallback(cardsWrap) {
      if (!cardsWrap) return;
      var Og = window.OgPreviewClient || {};
      var defaults = ogPreviewDefaults();
      var cards = cardsWrap.querySelectorAll(".public-post-card");
      cards.forEach(function (card) {
        if (card.getAttribute("data-judicial-feed-suppress") === "1") return;
        if (card.querySelector(".public-post-og-image")) return;
        if (card.querySelector(".public-post-og--image-withheld")) return;
        if (card.querySelector(".public-post-og-loading")) return;
        if (card.getAttribute("data-og-fetch-pending") === "1") return;
        var body = card.querySelector(".public-post-body");
        if (!body) return;
        var url = firstUrlFromFeedCard(card);
        if (!url || isPlatformSelfLink(url)) return;
        if (!card.getAttribute("data-link-url")) {
          card.setAttribute("data-link-url", url);
        }
        var existingOg = card.querySelector(".public-post-og");
        if (!existingOg) {
          insertFeedOgFragment(card, feedOgLoadingHtml());
        }
        card.setAttribute("data-og-fetch-pending", "1");
        fetch("/content-links/preview-og", {
          method: "POST",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify({ url: url }),
        })
          .then(function (r) {
            if (!r.ok) throw new Error("preview_failed");
            return r.json();
          })
          .then(function (data) {
            card.removeAttribute("data-og-fetch-pending");
            if (!data || data.domain_blocked) {
              var blockedLoading = card.querySelector(".public-post-og-loading");
              if (blockedLoading) blockedLoading.remove();
              return;
            }
            var canonical = (data.url_canonical || url || "").trim();
            if (!canonical) {
              var missingLoading = card.querySelector(".public-post-og-loading");
              if (missingLoading) missingLoading.remove();
              return;
            }
            var title = data.og_title || canonical;
            var site = data.og_site_name || "";
            var media =
              Og.mediaInnerFromPreview &&
              Og.mediaInnerFromPreview(data, escapeHtml, defaults);
            if (!media) {
              media = { inner: "", withheld: false, hasImage: false };
            }
            var ogRoot = card.querySelector(".public-post-og");
            if (ogRoot) {
              if (Og.applyOgPreviewToRoot) {
                Og.applyOgPreviewToRoot(ogRoot, data, escapeHtml, defaults);
              }
              var linkEl = card.querySelector(".public-post-og-link");
              if (linkEl) {
                linkEl.setAttribute("href", canonical);
                linkEl.setAttribute("aria-label", title || "Open link");
              }
              var doneLoading = card.querySelector(".public-post-og-loading");
              if (doneLoading) doneLoading.remove();
              scheduleFeedCardsMasonry();
              return;
            }
            var ogInner =
              Og.buildOgInnerHtml &&
              Og.buildOgInnerHtml(title, site, media, escapeHtml);
            if (!ogInner) {
              ogInner =
                '<div class="public-post-og public-post-og--no-image">' +
                '<div class="public-post-og-media" aria-hidden="true"></div>' +
                '<div class="public-post-og-copy"><div class="public-post-og-title">' +
                escapeHtml(title) +
                '</div><div class="public-post-og-site">' +
                escapeHtml(site) +
                "</div></div></div>";
            }
            var html =
              '<a class="public-post-og-link" href="' +
              escapeHtml(canonical) +
              '" target="_blank" rel="noopener noreferrer" aria-label="' +
              escapeHtml(title || "Open link") +
              '">' +
              ogInner +
              "</a>";
            var loadEl = card.querySelector(".public-post-og-loading");
            if (loadEl) {
              loadEl.insertAdjacentHTML("beforebegin", html);
              loadEl.remove();
            } else {
              insertFeedOgFragment(card, html);
            }
            scheduleFeedCardsMasonry();
          })
          .catch(function () {
            card.removeAttribute("data-og-fetch-pending");
            var errorLoading = card.querySelector(".public-post-og-loading");
            if (errorLoading) errorLoading.remove();
            scheduleFeedCardsMasonry();
          });
      });
    }

    function bindProfileConstitutionExpander(root) {
      if (!root) return;
      var body = root.querySelector("#profile-constitution-body");
      var btn = root.querySelector("[data-profile-constitution-expand]");
      if (!body || !btn) return;
      body.classList.add("is-clamped");
      var showMore =
        (UI && UI.constitution_show_more) || "Show more …";
      var showLess =
        (UI && UI.constitution_show_less) || "Show less";
      function syncButton() {
        var hasOverflow = body.scrollHeight > body.clientHeight + 1;
        btn.hidden = !hasOverflow;
        if (!hasOverflow) {
          body.classList.remove("is-clamped");
          body.classList.remove("is-expanded");
          btn.setAttribute("aria-expanded", "false");
          btn.textContent = showMore;
          return;
        }
        if (!body.classList.contains("is-expanded")) {
          body.classList.add("is-clamped");
        }
      }
      btn.addEventListener("click", function () {
        var expanded = body.classList.toggle("is-expanded");
        body.classList.toggle("is-clamped", !expanded);
        btn.setAttribute("aria-expanded", expanded ? "true" : "false");
        btn.textContent = expanded ? showLess : showMore;
      });
      syncButton();
      if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(syncButton);
      }
    }

    function bindFeedExpanders(cardsWrap) {
      if (!cardsWrap) return;
      cardsWrap.querySelectorAll(".app-feed-post-body").forEach(function (body) {
        body.classList.add("is-clamped");
      });
      cardsWrap.querySelectorAll(".app-feed-post-expand-btn").forEach(function (btn) {
        var targetId = (btn.getAttribute("data-feed-expand-target") || "").trim();
        if (!targetId) return;
        var body = document.getElementById(targetId);
        if (!body) return;
        var hasOverflow = body.scrollHeight > body.clientHeight + 1;
        btn.hidden = !hasOverflow;
        if (!hasOverflow) {
          body.classList.remove("is-clamped");
          return;
        }
        btn.addEventListener("click", function () {
          var expanded = body.classList.toggle("is-expanded");
          body.classList.toggle("is-clamped", !expanded);
          btn.textContent = expanded
            ? (UI.post_engagement_show_less || UI.constitution_show_less || "Show less")
            : (UI.post_engagement_show_more || UI.constitution_show_more || "Show more …");
          scheduleFeedCardsMasonry();
        });
      });
    }

    function hydrateFeedCountryTooltips(cardsWrap) {
      if (!cardsWrap || !window.Intl || !Intl.DisplayNames) return;
      var dn = null;
      try {
        dn = new Intl.DisplayNames([document.documentElement.lang || "en"], { type: "region" });
      } catch (_) {
        dn = null;
      }
      if (!dn) return;
      cardsWrap.querySelectorAll("[data-country-code].public-post-flag, [data-country-code].public-post-flag-icon").forEach(function (el) {
        var code = (el.getAttribute("data-country-code") || "").trim().toUpperCase();
        if (!/^[A-Z]{2}$/.test(code)) return;
        var country = "";
        try {
          country = dn.of(code) || "";
        } catch (_) {
          country = "";
        }
        if (!country) return;
        var postedFrom = (UI.feed_posted_from || "Posted from {country}").replace("{country}", country);
        el.setAttribute("title", postedFrom);
        el.setAttribute("aria-label", postedFrom);
      });
    }

    function showMentionsLoginNotice(count) {
      var n = Number(count);
      if (!Number.isFinite(n) || n <= 0) return;
      var msgTpl = (UI && UI.mentions_login_notice) || "You have {count} new mention(s) since your last visit.";
      var msg = msgTpl.replace("{count}", String(n));
      if (typeof window.showRightPanelNotice === "function") {
        window.showRightPanelNotice(msg);
      }
      fetch("/home/mentions/ack", {
        method: "POST",
        credentials: "same-origin",
        headers: { Accept: "application/json" },
      }).catch(function () {});
    }

    function updateJurorSummonsBadge(pending) {
      var rest = document.querySelector(".app-topbar-rest");
      if (!rest) return;
      var existing = rest.querySelector("[data-juror-summons-badge]");
      if (!pending) {
        if (existing) existing.remove();
        return;
      }
      if (existing) return;
      var label = (UI && UI.juror_summons_badge) || "Jury summons";
      var badge = document.createElement("a");
      badge.className = "app-topbar-juror-badge";
      badge.href = "/home";
      badge.setAttribute("data-juror-summons-badge", "1");
      badge.setAttribute("aria-label", label);
      badge.innerHTML =
        '<span class="app-topbar-juror-badge-dot" aria-hidden="true"></span><span class="app-topbar-juror-badge-text">' +
        label +
        "</span>";
      var titleEl = rest.querySelector(".app-topbar-title");
      if (titleEl) rest.insertBefore(badge, titleEl);
      else {
        var tokens = rest.querySelector(".app-topbar-ai-tokens");
        if (tokens) rest.insertBefore(badge, tokens);
        else rest.insertBefore(badge, rest.firstChild);
      }
    }

    function removeJurorSummonSlot() {
      var el = document.getElementById("app-juror-invite-slot");
      if (el) el.remove();
    }

    function bindJurorWorkspaceForms(mountEl) {
      if (!mountEl) return;
      mountEl.querySelectorAll("form[data-juror-vote-form]").forEach(function (f) {
        if (f.getAttribute("data-juror-vote-bound") === "1") return;
        f.setAttribute("data-juror-vote-bound", "1");
        f.addEventListener("submit", async function (e) {
          e.preventDefault();
          var btn = f.querySelector("button[type='submit']");
          if (btn) btn.disabled = true;
          try {
            var fd = new URLSearchParams(new FormData(f));
            var r = await fetch(f.getAttribute("action"), {
              method: "POST",
              credentials: "same-origin",
              headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
              body: fd.toString(),
            });
            if (!r.ok) throw new Error("vote_failed");
            enqueueRightPanelNotification(UI.juror_vote_recorded || "Vote recorded. Thank you.", 3200);
            removeJurorSummonSlot();
            loadFeedDay(currentFeedPage);
          } catch (_e) {
            enqueueRightPanelNotification(UI.juror_workspace_failed || "Could not record vote.", 3600);
          } finally {
            if (btn) btn.disabled = false;
          }
        });
      });
      mountEl
        .querySelectorAll("[data-law-escalate-stub], [data-juror-law-stub]")
        .forEach(function (btn) {
          if (btn.getAttribute("data-law-escalate-bound") === "1") return;
          btn.setAttribute("data-law-escalate-bound", "1");
          btn.addEventListener("click", function () {
            enqueueRightPanelNotification(UI.juror_law_notice || "", 5200);
          });
        });
    }

    function bindPendingInvitationCards(rootEl) {
      if (!rootEl) return;
      var selects = rootEl.querySelectorAll("[data-pending-invitation-select]");
      for (var i = 0; i < selects.length; i++) {
        (function (sel) {
          if (sel.getAttribute("data-pending-invitation-bound") === "1") return;
          sel.setAttribute("data-pending-invitation-bound", "1");
          var card = sel.closest("[data-pending-invitation]");
          var relId = card ? (card.getAttribute("data-relationship-id") || "").trim() : "";
          if (!relId) return;
          sel.addEventListener("change", async function () {
            var action = (sel.value || "").trim().toLowerCase();
            if (!action) return;
            sel.disabled = true;
            try {
              var fd = new URLSearchParams();
              fd.set("response_action", action);
              var r = await fetch(
                "/home/pending-invitation/" + encodeURIComponent(relId) + "/respond",
                {
                  method: "POST",
                  credentials: "same-origin",
                  headers: {
                    "Content-Type": "application/x-www-form-urlencoded",
                    Accept: "application/json",
                  },
                  body: fd.toString(),
                }
              );
              if (!r.ok) throw new Error("respond_failed");
              if (card) card.remove();
              var slot = document.getElementById("app-juror-invite-slot");
              if (slot && !slot.querySelector("[data-pending-invitation], [data-juror-summons]")) {
                removeJurorSummonSlot();
              }
              loadFeedDay(currentFeedPage);
            } catch (_e) {
              enqueueRightPanelNotification(
                UI.connect_send_failed || "Could not update invitation right now",
                3000
              );
              sel.value = "";
              sel.disabled = false;
            }
          });
        })(selects[i]);
      }
    }

    function bindJurorSummonSlot(rootEl) {
      if (!rootEl || rootEl.getAttribute("data-juror-summons-bound") === "1") return;
      rootEl.setAttribute("data-juror-summons-bound", "1");
      bindPendingInvitationCards(rootEl);
      /* `data-invite-id` lives on the inner summons `<section>`, not on `#app-juror-invite-slot` wrapper. */
      var inviteHost = rootEl.querySelector("[data-invite-id]") || rootEl;
      var inviteId = (inviteHost.getAttribute("data-invite-id") || "").trim();
      var sel = rootEl.querySelector("[data-juror-summons-select]");
      var actions = rootEl.querySelector(".juror-summons-actions");
      var mount = rootEl.querySelector("[data-juror-workspace-mount]");
      if (sel && mount && inviteId) {
        sel.addEventListener("change", async function () {
          var choice = (sel.value || "").trim().toLowerCase();
          if (!choice) return;
          if (choice === "abstain") {
            sel.disabled = true;
            try {
              var r = await fetch("/home/juror-invite/" + encodeURIComponent(inviteId) + "/abstain", {
                method: "POST",
                credentials: "same-origin",
                headers: { Accept: "application/json" },
              });
              if (!r.ok) throw new Error("ab");
              enqueueRightPanelNotification(UI.juror_abstain_recorded || "Abstention recorded.", 3200);
              removeJurorSummonSlot();
              loadFeedDay(currentFeedPage);
            } catch (_e) {
              enqueueRightPanelNotification(UI.juror_workspace_failed || "Could not record abstention.", 3600);
              sel.value = "";
            } finally {
              sel.disabled = false;
            }
            return;
          }
          if (choice === "accept") {
            sel.disabled = true;
            try {
              var wr = await fetch("/home/juror-invite/" + encodeURIComponent(inviteId) + "/workspace", {
                credentials: "same-origin",
                headers: { Accept: "text/html" },
              });
              if (!wr.ok) throw new Error("ws");
              var h = await wr.text();
              if (!h || !String(h).trim()) throw new Error("empty");
              mount.innerHTML = h;
              mount.hidden = false;
              if (actions) actions.hidden = true;
              bindJurorWorkspaceForms(mount);
              bindFeedExpanders(mount);
              if (typeof window.bindPostEngagementFeed === "function") {
                window.bindPostEngagementFeed(mount);
              }
              hydrateFeedOgFallback(mount);
              hydrateFeedCountryTooltips(mount);
            } catch (_e) {
              enqueueRightPanelNotification(UI.juror_workspace_failed || "Could not load jury workspace.", 3600);
              sel.value = "";
            } finally {
              sel.disabled = false;
            }
          }
        });
      }
    }

    async function refreshJurorSummonSlot(expectedNonce) {
      if (!feedPanel) return;
      if (currentProfileHandle) return;
      if (activeView !== "feed") return;
      if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) return;
      try {
        var r = await fetch("/home/juror-invite-panel", {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) return;
        removeJurorSummonSlot();
        if (!r.ok) return;
        var html = await r.text();
        if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) return;
        if (!html || !String(html).trim()) return;
        var wrap = document.createElement("div");
        wrap.id = "app-juror-invite-slot";
        wrap.className = "app-juror-invite-slot";
        wrap.innerHTML = html;
        feedPanel.insertBefore(wrap, feedPanel.firstChild);
        bindJurorSummonSlot(wrap);
      } catch (_e) {}
    }

    async function loadFeedDay(page, expectedNonce) {
      if (!feedPanel) return;
      if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) {
        return;
      }
      var loadGen = ++feedLoadGeneration;
      var scrollAfterLoad = pendingFeedScrollToTop;
      pendingFeedScrollToTop = false;
      var feedPage = clampFeedPage(page);
      if (!document.getElementById("app-feed-cards-wrap")) {
        renderFeedShell();
      }
      bindFeedControls();
      setFeedControlsLoading(true);
      setFeedCardsLoading(true);
      try {
        var feedParams = new URLSearchParams();
        feedParams.set("page", String(feedPage));
        if (currentProfileHandle) {
          var isSelfProfile =
            selfPrivateHandle &&
            normalizeHandleCandidate(currentProfileHandle) ===
              normalizeHandleCandidate(selfPrivateHandle);
          if (isSelfProfile) {
            feedParams.set("mentions_only", "1");
          } else {
            feedParams.set("profile_handle", currentProfileHandle);
          }
        }
        var r = await fetch(FEED_DAY_URL + "?" + feedParams.toString(), {
          credentials: "same-origin",
          cache: "no-store",
          headers: { Accept: "application/json" },
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          throw { rateMsg: d429.error || UI.card_rate_limited };
        }
        if (!r.ok) {
          var errBody = await r.json().catch(function () {
            return {};
          });
          var errMsg =
            (errBody && (errBody.error || errBody.message)) ||
            UI.feed_load_failed ||
            "Could not load feed.";
          throw { rateMsg: errMsg + " (HTTP " + String(r.status) + ")", httpStatus: r.status };
        }
        var data = await r.json();
        if (loadGen !== feedLoadGeneration) {
          return;
        }
        if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) {
          return;
        }
        var nomsCount = (data && typeof data.noms === "number") ? data.noms : null;
        if (nomsCount != null) {
          var nomsEl = document.querySelector(".app-topbar-ai-tokens .nom-amount-count");
          if (!nomsEl) {
            nomsEl = document.querySelector(".app-topbar-ai-tokens-count");
          }
          if (nomsEl) nomsEl.textContent = String(nomsCount);
          var nomsWrap = document.querySelector(".app-topbar-ai-tokens");
          if (nomsWrap && UI.noms_wallet_aria) {
            nomsWrap.setAttribute(
              "aria-label",
              String(UI.noms_wallet_aria).replace("{balance}", String(nomsCount))
            );
          } else if (nomsWrap) {
            nomsWrap.setAttribute("aria-label", "Noms: " + String(nomsCount));
          }
        }
        updateJurorSummonsBadge(data && data.juror_summons_pending === true);
        showMentionsLoginNotice(data && data.unseen_mention_count);
        currentFeedPage = clampFeedPage((data && data.page) || 0);
        updateFeedNavButtons(data || null);
        var cardsWrap = document.getElementById("app-feed-cards-wrap");
        if (cardsWrap) {
          cardsWrap.innerHTML = (data && data.cards_html) ? String(data.cards_html) : defaultFeedHtml();
          setFeedCardsLoading(false);
          try {
            hydrateFeedCards(cardsWrap);
          } catch (hydrateErr) {
            console.error("feed card hydrate failed", hydrateErr);
          }
        }
        refreshJurorSummonSlot(expectedNonce);
        syncFeedScrollTopButton();
      } catch (e) {
        if (loadGen !== feedLoadGeneration) {
          return;
        }
        if (expectedNonce != null && !isFeedPanelViewNonce(expectedNonce)) {
          return;
        }
        console.error("loadFeedDay failed", e);
        var cardsWrapErr = document.getElementById("app-feed-cards-wrap");
        if (cardsWrapErr) {
          cardsWrapErr.innerHTML =
            '<section class="card app-card"><div class="card-accent" role="presentation"></div><p class="sub" role="alert"></p></section>';
          var msgEl = cardsWrapErr.querySelector("p[role='alert']");
          if (msgEl) {
            msgEl.textContent =
              (e && e.rateMsg) ||
              (e && e.message) ||
              UI.feed_load_failed;
          }
        }
      } finally {
        if (loadGen === feedLoadGeneration) {
          setFeedControlsLoading(false);
          setFeedCardsLoading(false);
          syncFeedScrollTopButton();
          if (scrollAfterLoad) {
            scrollFeedAfterPageLoad();
          }
        }
      }
    }

    async function openProfilePanel() {
      var viewNonce = beginView("onboarding-profile");
      if (feedPanel) feedPanel.hidden = true;
      syncFeedScrollTopButton();
      if (createPanel) createPanel.hidden = true;
      profilePanel.hidden = false;
      loadingEl.hidden = false;
      mountEl.innerHTML = "";
      if (loadingMsg) {
        loadingMsg.textContent = "";
        loadingMsg.hidden = true;
      }

      try {
        var r = await fetch(CARD_URL, {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          throw { rateMsg: d429.error || UI.card_rate_limited };
        }
        if (!r.ok) throw new Error("load_failed");
        if (!isCurrentView("onboarding-profile", viewNonce)) return;
        var html = await r.text();
        if (!isCurrentView("onboarding-profile", viewNonce)) return;
        mountEl.innerHTML = html;
        loadingEl.hidden = true;
        bindProfileForm(UI);
        startProfileCountryDetection();
      } catch (e) {
        if (!isCurrentView("onboarding-profile", viewNonce)) return;
        if (loadingMsg) {
          loadingMsg.textContent = e && e.rateMsg ? e.rateMsg : UI.profile_load_failed;
          loadingMsg.hidden = false;
        }
      }
    }

    function applyPrivateProfileTitle(handle) {
      if (!handle) return;
      var atHandle = "@" + handle;
      if (UI.private_profile_doc_title_suffix) {
        document.title = atHandle + " " + UI.private_profile_doc_title_suffix;
      }
      var topbarTitle = document.querySelector(".app-topbar-title");
      if (!topbarTitle) return;
      var tagline = topbarTitle.querySelector(".app-topbar-home-tagline");
      var handleEl = topbarTitle.querySelector(".app-topbar-home-handle");
      var display = topbarTitle.querySelector(".app-topbar-display");
      if (handleEl) handleEl.textContent = atHandle;
      if (tagline) tagline.textContent = atHandle;
      if (display) display.textContent = atHandle;
    }

    function bindPrivateProfileConnectForm() {
      var form = document.getElementById("profile-connect-form");
      var select = document.getElementById("profile-connect-as");
      if (!form || !select || form.getAttribute("data-connect-bound") === "1") return;
      form.setAttribute("data-connect-bound", "1");
      var isMembershipReview = form.getAttribute("data-membership-review") === "1";
      var editor = document.getElementById("profile-membership-review-editor");
      var statementInput = document.getElementById("profile-membership-review-statement-input");
      var toolbar = form.querySelector(".create-post-editor-toolbar");
      if (toolbar && editor) {
        toolbar.addEventListener("click", function (e) {
          var btn = e.target && e.target.closest ? e.target.closest("[data-cmd]") : null;
          if (!btn) return;
          e.preventDefault();
          var cmd = btn.getAttribute("data-cmd");
          if (!cmd) return;
          editor.focus();
          var value = btn.getAttribute("data-value");
          document.execCommand(cmd, false, value != null ? value : null);
        });
      }
      select.addEventListener("change", async function () {
        var connectAs = (select.value || "").trim().toLowerCase();
        if (!connectAs) return;
        if (isMembershipReview && connectAs === "member") {
          var markup = editor ? String(editor.innerHTML || "").trim() : "";
          var plain = editor
            ? String(editor.innerText || editor.textContent || "")
                .replace(/\r\n/g, "\n")
                .trim()
            : "";
          if (!plain) {
            enqueueRightPanelNotification(
              UI.membership_review_statement_required ||
                "Write a short statement for the community jury.",
              3200
            );
            select.value = "";
            return;
          }
          if (statementInput) statementInput.value = markup;
        }
        select.disabled = true;
        try {
          var fd = new URLSearchParams();
          fd.set("connect_as", connectAs);
          if (isMembershipReview && statementInput) {
            fd.set("application_statement", statementInput.value || "");
          }
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
            body: fd.toString(),
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            var errMsg =
              (payload && (payload.error || payload.message)) ||
              UI.connect_send_failed ||
              "Could not send invitation right now";
            throw new Error(String(errMsg));
          }
          var notice =
            connectAs === "block"
              ? (payload && payload.message) || UI.block_success || "Blocked."
              : (payload && payload.message) ||
                UI.connect_invitation_sent ||
                "Connection Invitation Sent";
          if (connectAs === "block") {
            var card = document.getElementById("private-profile-card");
            if (card && card.parentNode) card.parentNode.removeChild(card);
            enqueueRightPanelNotification(notice, 3000);
            return;
          }
          var block = document.getElementById("profile-connect-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-connect-sent-notice">' +
              escapeHtml(String(notice)) +
              "</p>";
          }
          enqueueRightPanelNotification(notice, 3000);
        } catch (e) {
          var failMsg =
            (e && e.message && String(e.message) !== "connect_failed" && e.message) ||
            UI.connect_send_failed ||
            "Could not send invitation right now";
          enqueueRightPanelNotification(failMsg, 3000);
          select.value = "";
          select.disabled = false;
        }
      });
    }

    function bindFeedJudicialWithdrawForms(cardsWrap) {
      if (!cardsWrap || !cardsWrap.querySelectorAll) return;
      var forms = cardsWrap.querySelectorAll("form.public-post-judicial-withdraw-form");
      for (var i = 0; i < forms.length; i++) {
        var form = forms[i];
        if (form.getAttribute("data-judicial-withdraw-bound") === "1") continue;
        var select = form.querySelector("select[name='withdraw_action']");
        if (!select) continue;
        form.setAttribute("data-judicial-withdraw-bound", "1");
        (function (formEl, selectEl) {
          selectEl.addEventListener("change", async function () {
            var action = (selectEl.value || "").trim().toLowerCase();
            if (!action) return;
            selectEl.disabled = true;
            try {
              var fd = new URLSearchParams();
              fd.set("withdraw_action", action);
              var r = await fetch(formEl.getAttribute("action"), {
                method: "POST",
                credentials: "same-origin",
                headers: {
                  "Content-Type": "application/x-www-form-urlencoded",
                  Accept: "application/json",
                },
                body: fd.toString(),
              });
              if (!r.ok) throw new Error("withdraw_failed");
              var card = formEl.closest("section.public-post-card");
              if (card && card.parentNode) card.parentNode.removeChild(card);
              enqueueRightPanelNotification(
                UI.judicial_withdraw_success ||
                  "Post withdrawn from the feed and the judicial case was closed.",
                3200,
              );
            } catch (_e) {
              enqueueRightPanelNotification(
                UI.judicial_withdraw_failed || "Could not withdraw that post right now.",
                3200,
              );
              selectEl.value = "";
              selectEl.disabled = false;
            }
          });
        })(form, select);
      }
    }

    function bindPrivateProfileInviteResponseForm() {
      var form = document.getElementById("profile-invite-response-form");
      var select = document.getElementById("profile-invite-response");
      if (!form || !select || form.getAttribute("data-invite-response-bound") === "1") return;
      form.setAttribute("data-invite-response-bound", "1");
      select.addEventListener("change", async function () {
        var responseAction = (select.value || "").trim().toLowerCase();
        if (!responseAction) return;
        select.disabled = true;
        try {
          var fd = new URLSearchParams();
          fd.set("response_action", responseAction);
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
            body: fd.toString(),
          });
          if (!r.ok) throw new Error("invite_response_failed");
          var msg =
            responseAction === "accept"
              ? UI.invite_response_accepted
              : responseAction === "decline"
                ? UI.invite_response_declined
                : UI.invite_response_blocked;
          if (responseAction === "block") {
            var card = document.getElementById("private-profile-card");
            if (card && card.parentNode) card.parentNode.removeChild(card);
            enqueueRightPanelNotification(msg || UI.block_success || "Blocked.", 3000);
            return;
          }
          var block = document.getElementById("profile-connect-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-invite-response-notice">' +
              (msg || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not update connection right now", 3000);
          select.value = "";
          select.disabled = false;
        }
      });
    }

    function bindPrivateProfileCancelInviteForm() {
      var form = document.getElementById("profile-cancel-invite-form");
      if (!form || form.getAttribute("data-cancel-bound") === "1") return;
      form.setAttribute("data-cancel-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("cancel_failed");
          var block = document.getElementById("profile-connect-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-invite-cancelled-notice">' +
              (UI.invitation_cancelled_notice || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not cancel invitation right now", 3000);
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindPrivateProfileBlockForm() {
      var form = document.getElementById("profile-block-form");
      if (!form || form.getAttribute("data-block-bound") === "1") return;
      form.setAttribute("data-block-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            throw new Error(
              (payload && (payload.error || payload.message)) || "block_failed"
            );
          }
          var msg = (payload && payload.message) || UI.block_success || "Blocked.";
          var card = document.getElementById("private-profile-card");
          if (card && card.parentNode) card.parentNode.removeChild(card);
          enqueueRightPanelNotification(msg, 3000);
        } catch (_e) {
          enqueueRightPanelNotification(
            UI.connect_send_failed || "Could not block that profile right now",
            3000
          );
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindPrivateProfileDisconnectForm() {
      var form = document.getElementById("profile-disconnect-form");
      if (!form || form.getAttribute("data-disconnect-bound") === "1") return;
      form.setAttribute("data-disconnect-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("disconnect_failed");
          var block = document.getElementById("profile-connect-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-disconnected-notice">' +
              (UI.disconnected_notice || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not disconnect right now", 3000);
          if (btn) btn.disabled = false;
        }
      });
    }

    function waitForCondition(testFn, timeoutMs, label) {
      return new Promise(function (resolve, reject) {
        var started = Date.now();
        (function tick() {
          try {
            if (testFn()) {
              resolve();
              return;
            }
          } catch (_) {
            /* keep polling */
          }
          if (Date.now() - started > timeoutMs) {
            reject(new Error(label || "waitForCondition timeout"));
            return;
          }
          window.requestAnimationFrame(tick);
        })();
      });
    }

    function scriptLoadSettled(scriptEl) {
      if (!scriptEl) return false;
      if (scriptEl.getAttribute("data-loaded") === "1") return true;
      var src = scriptEl.getAttribute("src") || "";
      if (src.indexOf("face-api.min.js") >= 0) {
        return typeof faceapi !== "undefined" || !!(window.faceapi || (typeof globalThis !== "undefined" && globalThis.faceapi));
      }
      if (src.indexOf("face-capture.js") >= 0) {
        return !!(window.AppFaceCapture && typeof window.AppFaceCapture.bootstrap === "function");
      }
      return false;
    }

    function loadScriptOnce(src) {
      return new Promise(function (resolve, reject) {
        var existing = document.querySelector('script[src="' + src + '"]');
        if (existing) {
          if (scriptLoadSettled(existing)) {
            existing.setAttribute("data-loaded", "1");
            resolve();
            return;
          }
          existing.addEventListener(
            "load",
            function onLoad() {
              existing.removeEventListener("load", onLoad);
              existing.setAttribute("data-loaded", "1");
              resolve();
            },
            { once: true }
          );
          existing.addEventListener(
            "error",
            function onErr(e) {
              existing.removeEventListener("error", onErr);
              reject(e);
            },
            { once: true }
          );
          return;
        }
        var s = document.createElement("script");
        s.src = src;
        s.async = false;
        s.addEventListener(
          "load",
          function () {
            s.setAttribute("data-loaded", "1");
            resolve();
          },
          { once: true }
        );
        s.addEventListener(
          "error",
          function (e) {
            reject(e);
          },
          { once: true }
        );
        document.head.appendChild(s);
      });
    }

    async function ensureFaceCaptureReady() {
      await loadScriptOnce("/static/vendor/face-api/face-api.min.js");
      await waitForCondition(function () {
        return typeof faceapi !== "undefined" || !!(window.faceapi || (typeof globalThis !== "undefined" && globalThis.faceapi));
      }, 10000, "face_api_global");
      await loadScriptOnce("/static/js/face-capture.js?v=20260811-redraw-models2");
      await waitForCondition(function () {
        return !!(window.AppFaceCapture && typeof window.AppFaceCapture.preloadModels === "function");
      }, 10000, "face_capture_module");
      await window.AppFaceCapture.preloadModels();
      return window.AppFaceCapture;
    }

    function nomAmountHtml(amount) {
      var icon = UI.nom_icon_sm || "";
      return (
        '<span class="nom-amount"><span class="nom-amount-count mono">' +
        String(amount) +
        "</span>" +
        icon +
        "</span>"
      );
    }

    function formatNomInsufficient(template, needed, balance) {
      return String(template || "")
        .replace(/\{nom_price\}/g, nomAmountHtml(needed))
        .replace(/\{needed\}/g, String(needed))
        .replace(/\{balance\}/g, String(balance));
    }

    async function openProfileAvatarRedrawCapture() {
      if (!currentProfileHandle || !feedPanel) return;
      var slot = document.getElementById("app-private-profile-card-slot");
      if (!slot) return;
      var captureNonce = beginView("private-profile");
      scrollFeedToTop();
      slot.innerHTML = panelCardLoaderHtml("face-capture");
      try {
        var r = await fetch(FACE_CAPTURE_CARD_URL + "?mode=avatar_redraw", {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (!r.ok) throw new Error("face_capture_card_failed");
        if (!isCurrentView("private-profile", captureNonce)) return;
        slot.innerHTML = await r.text();
        if (!isCurrentView("private-profile", captureNonce)) return;
        var FC = await ensureFaceCaptureReady();
        if (!FC || typeof FC.bootstrap !== "function") throw new Error("face_capture_bootstrap_missing");
        var capture = FC.bootstrap({
          root: slot,
          suppressInlineSuccess: true,
          onComplete: function () {
            if (!isCurrentView("private-profile", captureNonce)) return;
            enqueueRightPanelNotification(
              UI.avatar_redraw_success || "New avatar saved.",
              3200
            );
            openPrivateProfileCard(currentProfileHandle, false, true);
            scrollFeedToTop();
          },
          onCancel: function () {
            if (!isCurrentView("private-profile", captureNonce)) return;
            openPrivateProfileCard(currentProfileHandle, false, true);
            scrollFeedToTop();
          },
        });
        if (!capture) throw new Error("face_capture_bootstrap_rejected");
        scrollFeedToTop();
      } catch (_e) {
        if (!isCurrentView("private-profile", captureNonce)) return;
        enqueueRightPanelNotification(
          UI.avatar_redraw_failed || "Could not open face capture right now.",
          4000
        );
        openPrivateProfileCard(currentProfileHandle, false, true);
      }
    }

    function bindPrivateProfileAvatarRedrawButton(root) {
      root =
        root ||
        document.getElementById("app-private-profile-card-slot") ||
        document.querySelector('[data-card-profile="private"]');
      if (!root) return;
      var btn = root.querySelector("#profile-avatar-redraw-btn");
      if (!btn || btn.getAttribute("data-avatar-redraw-bound") === "1") return;
      btn.setAttribute("data-avatar-redraw-bound", "1");
      btn.addEventListener("click", function () {
        var canAfford = btn.getAttribute("data-avatar-redraw-can-afford") === "1";
        if (!canAfford) {
          var needed = Number(btn.getAttribute("data-avatar-redraw-cost") || 16);
          var balance = Number(btn.getAttribute("data-avatar-redraw-balance") || 0);
          enqueueRightPanelNotification(
            formatNomInsufficient(
              UI.avatar_redraw_insufficient_noms ||
                "You need at least {needed} Noms to redraw your avatar (you have {balance}).",
              needed,
              balance
            ),
            4500
          );
          return;
        }
        openProfileAvatarRedrawCapture();
      });
    }

    var profileAccountPanels = null;

    function bindPrivateProfileHumanEditForm(root) {
      var profileCard = document.querySelector('[data-card-profile="private"]');
      root =
        root ||
        document.getElementById("app-private-profile-card-slot") ||
        (profileCard && profileCard.closest
          ? profileCard.closest(".app-private-profile-card-slot")
          : null) ||
        profileCard;
      if (!root) return;
      if (root.getAttribute("data-profile-human-edit-bound") === "1") return;

      var toggle = root.querySelector("#profile-human-edit-toggle");
      var block = root.querySelector("#profile-human-edit-block");
      var form = root.querySelector("#profile-human-edit-form");
      var cancelBtn = root.querySelector("#profile-human-edit-cancel");
      var msg = root.querySelector("#profile-human-edit-msg");
      if (!toggle || !block || !form) return;

      profileAccountPanels = window.ProfileAccount
        ? window.ProfileAccount.bind({
            root: root,
            block: block,
            editPanel: root.querySelector("#profile-human-edit-panel"),
            passwordBlock: root.querySelector("#profile-change-password-block"),
            passwordVerifyStep: root.querySelector("#profile-change-password-step-verify"),
            passwordNewStep: root.querySelector("#profile-change-password-step-new"),
            passwordMsg: root.querySelector("#profile-change-password-msg"),
            blocksBlock: root.querySelector("#profile-manage-blocks-block"),
            blocksList: root.querySelector("#profile-block-list"),
            blocksEmpty: root.querySelector("#profile-block-list-empty"),
            blocksMsg: root.querySelector("#profile-block-list-msg"),
            deleteBlock: root.querySelector("#profile-delete-account-block"),
            deleteMsg: root.querySelector("#profile-delete-account-msg"),
            scrollFeedToTop: scrollFeedToTop,
            notify: enqueueRightPanelNotification,
            UI: UI,
          })
        : null;

      function clearMsg(el) {
        if (!el) return;
        el.hidden = true;
        el.textContent = "";
      }

      function showMsg(el, text) {
        if (!el) return;
        el.hidden = !text;
        el.textContent = text || "";
      }

      function resetEditPanels() {
        if (profileAccountPanels && profileAccountPanels.resetAccountPanels) {
          profileAccountPanels.resetAccountPanels();
        }
        clearMsg(msg);
      }

      function setEditOpen(open) {
        block.hidden = !open;
        toggle.setAttribute("aria-expanded", open ? "true" : "false");
        var detailSections = root.querySelector("#profile-human-detail-sections");
        if (detailSections) detailSections.hidden = open;
        if (!open) resetEditPanels();
        else clearMsg(msg);
      }

      toggle.addEventListener("click", function () {
        if (block.hidden) resetEditPanels();
        setEditOpen(block.hidden);
      });
      if (cancelBtn) {
        cancelBtn.addEventListener("click", function () {
          setEditOpen(false);
        });
      }

      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var saveBtn = root.querySelector("#profile-human-edit-save");
        if (saveBtn) saveBtn.disabled = true;
        clearMsg(msg);
        try {
          var fd = new FormData(form);
          var draft = {
            given_name: (fd.get("given_name") || "").toString().trim(),
            family_name: (fd.get("family_name") || "").toString().trim(),
            additional_name: (fd.get("additional_name") || "").toString().trim() || null,
            country_code: (fd.get("country_code") || "").toString().trim().toUpperCase(),
            region: (fd.get("region") || "").toString().trim() || null,
            locality: (fd.get("locality") || "").toString().trim() || null,
            public_location_label: (fd.get("public_location_label") || "").toString().trim() || null,
          };
          var r = await fetch("/me/human-profile", {
            method: "PATCH",
            credentials: "same-origin",
            headers: { "Content-Type": "application/json", Accept: "application/json" },
            body: JSON.stringify(draft),
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            throw new Error(
              (payload && payload.error) ||
                UI.profile_edit_failed ||
                "Could not update profile right now"
            );
          }
          enqueueRightPanelNotification(
            UI.profile_edit_success || "Profile updated.",
            3200
          );
          setEditOpen(false);
          if (currentProfileHandle) {
            openPrivateProfileCard(currentProfileHandle, false, true);
            scrollFeedToTop();
          }
        } catch (err) {
          showMsg(
            msg,
            (err && err.message) || UI.profile_edit_failed || "Could not update profile right now"
          );
          enqueueRightPanelNotification(
            (err && err.message) || UI.profile_edit_failed || "Could not update profile right now",
            4000
          );
        } finally {
          if (saveBtn) saveBtn.disabled = false;
        }
      });

      root.setAttribute("data-profile-human-edit-bound", "1");
    }

    function bindCommunityInviteSearch(root) {
      var block = root ? root.querySelector("#profile-community-invite-block") : null;
      if (!block || block.getAttribute("data-community-invite-bound") === "1") return;
      block.setAttribute("data-community-invite-bound", "1");
      var communityHandle = String(block.getAttribute("data-community-handle") || "").trim();
      var searchInput = document.getElementById("profile-community-invite-search");
      var hiddenHandle = document.getElementById("profile-community-invite-handle");
      var results = document.getElementById("profile-community-invite-results");
      var status = document.getElementById("profile-community-invite-search-status");
      var submitBtn = document.getElementById("profile-community-invite-submit");
      var debounceTimer = null;

      function setStatus(msg, isError) {
        if (!status) return;
        var t = String(msg || "").trim();
        status.hidden = !t;
        status.textContent = t;
        status.classList.toggle("create-post-submit-status--error", !!isError && !!t);
      }

      function clearPick() {
        if (hiddenHandle) hiddenHandle.value = "";
        if (submitBtn) submitBtn.disabled = true;
        if (results) results.innerHTML = "";
      }

      function pickHandle(handle) {
        if (hiddenHandle) hiddenHandle.value = handle;
        if (searchInput) searchInput.value = "@" + handle;
        if (results) results.innerHTML = "";
        if (submitBtn) submitBtn.disabled = !handle;
        setStatus("", false);
      }

      async function runSearch(q) {
        if (!communityHandle) {
          setStatus(UI.community_invite_search_no_results || "Community not found.", true);
          return;
        }
        var params = new URLSearchParams();
        params.set("q", q);
        params.set("exclude_community_member", communityHandle);
        try {
          var r = await fetch(SEARCH_ENTITIES_URL + "?" + params.toString(), {
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (r.status === 429) {
            var rateBody = await r.json().catch(function () {
              return {};
            });
            setStatus((rateBody && rateBody.error) || UI.card_rate_limited || "Too many requests.", true);
            return;
          }
          if (!r.ok) throw new Error("search_failed");
          var items = await r.json();
          if (!results) return;
          if (!Array.isArray(items) || !items.length) {
            results.innerHTML = "";
            setStatus(UI.community_invite_search_no_results || "No matches.", false);
            return;
          }
          setStatus("", false);
          results.innerHTML = items
            .map(function (it) {
              return renderSearchAddResultRow(it, {
                asButton: true,
                buttonClass: "profile-community-invite-pick-btn",
              });
            })
            .join("");
        } catch (_e) {
          setStatus(UI.search_add_search_failed || "Search failed.", true);
        }
      }

      if (searchInput) {
        searchInput.addEventListener("input", function () {
          clearPick();
          var q = String(searchInput.value || "").trim();
          if (debounceTimer) window.clearTimeout(debounceTimer);
          if (q.length < 3) {
            setStatus(UI.handle_min_chars || "", false);
            return;
          }
          debounceTimer = window.setTimeout(function () {
            runSearch(q);
          }, 280);
        });
      }
      if (results) {
        results.addEventListener("click", function (e) {
          var btn = e.target && e.target.closest ? e.target.closest(".profile-community-invite-pick-btn") : null;
          if (!btn) return;
          e.preventDefault();
          pickHandle(String(btn.getAttribute("data-handle") || "").trim());
        });
      }
    }

    function bindCommunityInviteSendForm() {
      var form = document.getElementById("profile-community-invite-form");
      if (!form || form.getAttribute("data-community-invite-send-bound") === "1") return;
      form.setAttribute("data-community-invite-send-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var hidden = document.getElementById("profile-community-invite-handle");
        var handle = hidden ? String(hidden.value || "").trim() : "";
        if (!handle) return;
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var fd = new URLSearchParams();
          fd.set("invitee_handle", handle);
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
            body: fd.toString(),
          });
          if (!r.ok) throw new Error("invite_failed");
          var block = document.getElementById("profile-community-invite-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-community-invite-sent-notice">' +
              (UI.community_invite_sent || "Invitation sent.") +
              "</p>";
          }
          enqueueRightPanelNotification(UI.community_invite_sent || "Invitation sent.", 3000);
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not send invitation right now", 3000);
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindCommunityInviteResponseForm() {
      var form = document.getElementById("profile-community-invite-response-form");
      var select = document.getElementById("profile-community-invite-response");
      if (!form || !select || form.getAttribute("data-community-invite-response-bound") === "1") return;
      form.setAttribute("data-community-invite-response-bound", "1");
      select.addEventListener("change", async function () {
        var responseAction = (select.value || "").trim().toLowerCase();
        if (!responseAction) return;
        select.disabled = true;
        try {
          var fd = new URLSearchParams();
          fd.set("response_action", responseAction);
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
            body: fd.toString(),
          });
          if (!r.ok) throw new Error("invite_response_failed");
          var block = document.getElementById("profile-connect-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value">' +
              (responseAction === "accept"
                ? UI.invite_response_accepted
                : UI.invite_response_declined) +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not update invitation right now", 3000);
          select.value = "";
          select.disabled = false;
        }
      });
    }

    function bindPrivateProfileLeaveCommunityForm() {
      var form = document.getElementById("profile-leave-community-form");
      if (!form || form.getAttribute("data-leave-community-bound") === "1") return;
      form.setAttribute("data-leave-community-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("leave_community_failed");
          var block = document.getElementById("profile-leave-community-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-community-left-notice">' +
              (UI.community_left_notice || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(
            UI.connect_send_failed || "Could not leave this community right now",
            3000
          );
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindPrivateProfileFollowForm() {
      var form = document.getElementById("profile-follow-form");
      if (!form || form.getAttribute("data-follow-bound") === "1") return;
      form.setAttribute("data-follow-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("follow_failed");
          var block = document.getElementById("profile-entity-follow-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-follow-saved-notice">' +
              (UI.following_saved_notice || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not follow right now", 3000);
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindPrivateProfileUnfollowForm() {
      var form = document.getElementById("profile-unfollow-form");
      if (!form || form.getAttribute("data-unfollow-bound") === "1") return;
      form.setAttribute("data-unfollow-bound", "1");
      form.addEventListener("submit", async function (e) {
        e.preventDefault();
        var btn = form.querySelector("button[type='submit']");
        if (btn) btn.disabled = true;
        try {
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("unfollow_failed");
          var block = document.getElementById("profile-entity-follow-block");
          if (block) {
            block.innerHTML =
              '<p class="profile-value" id="profile-unfollow-saved-notice">' +
              (UI.unfollowed_notice || "") +
              "</p>";
          }
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not unfollow right now", 3000);
          if (btn) btn.disabled = false;
        }
      });
    }

    function bindPrivateProfileGatheringRsvpForm() {
      var form = document.getElementById("profile-gathering-rsvp-form");
      var select = document.getElementById("profile-gathering-rsvp");
      if (!form || !select || form.getAttribute("data-gathering-rsvp-bound") === "1") return;
      form.setAttribute("data-gathering-rsvp-bound", "1");
      select.addEventListener("change", async function () {
        var v = (select.value || "").trim().toLowerCase();
        if (!v) return;
        select.disabled = true;
        try {
          var fd = new URLSearchParams();
          fd.set("gathering_rsvp", v);
          var r = await fetch(form.getAttribute("action"), {
            method: "POST",
            credentials: "same-origin",
            headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
            body: fd.toString(),
          });
          if (!r.ok) throw new Error("gathering_rsvp_failed");
          enqueueRightPanelNotification(UI.gathering_rsvp_saved_notice || "Saved", 2600);
        } catch (_e) {
          enqueueRightPanelNotification(UI.connect_send_failed || "Could not save RSVP right now", 3000);
          select.value = "";
        } finally {
          select.disabled = false;
        }
      });
    }

    function profileCardDeps() {
      return {
        UI: UI,
        feedPanel: feedPanel,
        escapeHtml: escapeHtml,
        notify: enqueueRightPanelNotification,
        scrollFeedToTop: scrollFeedToTop,
        renderFeedShell: renderFeedShell,
        bindFeedControls: bindFeedControls,
        loadFeedDay: loadFeedDay,
        ensureRightPanelNotificationHost: ensureRightPanelNotificationHost,
        applyPrivateProfileTitle: applyPrivateProfileTitle,
        openPrivateProfileCard: openPrivateProfileCard,
        showHomeFeedPanel: showHomeFeedPanel,
        get currentProfileHandle() {
          return currentProfileHandle;
        },
        bindCommunityInviteSearch: bindCommunityInviteSearch,
        bindCommunityInviteSendForm: bindCommunityInviteSendForm,
        bindCommunityInviteResponseForm: bindCommunityInviteResponseForm,
        openProfileAvatarRedrawCapture: openProfileAvatarRedrawCapture,
      };
    }

    function mountPrivateProfileCardHtml(html, handle, viewNonce, pushUrl) {
      if (window.ProfileCard && typeof window.ProfileCard.mount === "function") {
        window.ProfileCard.mount(profileCardDeps(), html, handle, viewNonce, pushUrl);
        return;
      }
      renderFeedShell();
      var existingPrivateCard = document.getElementById("app-private-profile-card-slot");
      if (existingPrivateCard) existingPrivateCard.remove();
      var privateCardSlot = document.createElement("div");
      privateCardSlot.id = "app-private-profile-card-slot";
      privateCardSlot.className = "app-private-profile-card-slot";
      privateCardSlot.innerHTML = html;
      feedPanel.insertBefore(privateCardSlot, feedPanel.firstChild);
      bindPrivateProfileHumanEditForm(privateCardSlot);
    }

    function removePrivateProfileLoader() {
      var loader = document.getElementById("app-private-profile-loading-slot");
      if (loader) loader.remove();
    }

    function removePrivateProfileCardSlot() {
      var slot = document.getElementById("app-private-profile-card-slot");
      if (slot) slot.remove();
    }

    async function openPrivateProfileCard(handle, pushUrl, forceReload) {
      handle = normalizeHandleCandidate(handle);
      if (!handle || !feedPanel) return;

      if (
        !forceReload &&
        currentProfileHandle === handle &&
        document.getElementById("app-private-profile-card-slot")
      ) {
        bindPrivateProfileHumanEditForm(document.getElementById("app-private-profile-card-slot"));
        applyPrivateProfileTitle(handle);
        if (pushUrl && window.history && window.history.pushState) {
          window.history.pushState(
            { panel: "private-profile", handle: handle },
            "",
            "/home/profile/" + encodeURIComponent(handle)
          );
        }
        return;
      }

      var embeddedCard = feedPanel.querySelector('[data-card-profile="private"]');
      if (embeddedCard && !document.getElementById("app-feed-cards-wrap")) {
        currentProfileHandle = String(handle);
        currentFeedPage = 0;
        var embedNonce = beginView("private-profile");
        profilePanel.hidden = true;
        if (createPanel) createPanel.hidden = true;
        feedPanel.hidden = false;
        removeJurorSummonSlot();
        renderFeedShell();
        var embeddedSlot = feedPanel.querySelector('[data-card-profile="private"]');
        if (embeddedSlot && window.ProfileCard && window.ProfileCard.initEmbeddedSlot) {
          window.ProfileCard.initEmbeddedSlot(profileCardDeps(), embeddedSlot, handle, embedNonce, pushUrl);
        } else if (embeddedCard) {
          mountPrivateProfileCardHtml(embeddedCard.outerHTML, handle, embedNonce, pushUrl);
        }
        return;
      }

      currentProfileHandle = String(handle);
      currentFeedPage = 0;
      var viewNonce = beginView("private-profile");
      profilePanel.hidden = true;
      if (createPanel) createPanel.hidden = true;
      feedPanel.hidden = false;
      removeJurorSummonSlot();
      removePrivateProfileCardSlot();
      removePrivateProfileLoader();
      feedPanel.insertAdjacentHTML(
        "afterbegin",
        '<div id="app-private-profile-loading-slot">' +
          panelCardLoaderHtml("home-private-profile") +
          "</div>"
      );
      ensureRightPanelNotificationHost();
      try {
        var r = await fetch(PRIVATE_PROFILE_CARD_BASE + encodeURIComponent(handle), {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          throw { rateMsg: d429.error || UI.card_rate_limited };
        }
        if (!r.ok) throw new Error("load_failed");
        if (!isCurrentView("private-profile", viewNonce)) return;
        var html = await r.text();
        if (!isCurrentView("private-profile", viewNonce)) return;
        removePrivateProfileLoader();
        mountPrivateProfileCardHtml(html, handle, viewNonce, pushUrl);
      } catch (e) {
        if (!isCurrentView("private-profile", viewNonce)) return;
        removePrivateProfileLoader();
        currentProfileHandle = "";
        showHomeFeedPanel();
        enqueueRightPanelNotification(
          e && e.rateMsg ? e.rateMsg : UI.private_profile_load_failed,
          4200
        );
      }
    }

    window.openPrivateProfileCard = openPrivateProfileCard;
    window.openCreatePanel = openCreatePanel;

    async function openCreatePanel(preset) {
      if (!feedPanel) return;
      currentProfileHandle = "";
      pendingCreatePostPreset = preset || null;
      var viewNonce = beginView("feed-create");
      profilePanel.hidden = true;
      if (createPanel) createPanel.hidden = true;
      feedPanel.hidden = false;
      removeJurorSummonSlot();

      var existingCreate = document.getElementById("app-feed-create-slot");
      if (existingCreate) existingCreate.remove();
      var existingSearch = document.getElementById("app-feed-search-add-slot");
      if (existingSearch) existingSearch.remove();
      setRailSearchActive(false);

      var hadFeedShell = !!document.getElementById("app-feed-cards-wrap");
      if (!hadFeedShell) {
        renderFeedShell();
        bindFeedControls();
        loadFeedDay(currentFeedPage, viewNonce);
      } else {
        bindFeedControls();
      }

      var slot = document.createElement("div");
      slot.id = "app-feed-create-slot";
      slot.className = "app-feed-create-slot";
      slot.innerHTML = window.brandCardLoader ? window.brandCardLoader.cardLoaderHtml() : "";
      feedPanel.insertBefore(slot, feedPanel.firstChild);
      scrollFeedToTop(false);

      try {
        var r = await fetch(CREATE_POST_CARD_URL, {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          throw { rateMsg: d429.error || UI.card_rate_limited };
        }
        if (!r.ok) throw new Error("load_failed");
        if (!isCurrentView("feed-create", viewNonce)) return;
        var html = await r.text();
        if (!isCurrentView("feed-create", viewNonce)) return;
        if (!document.getElementById("app-feed-create-slot")) {
          feedPanel.insertBefore(slot, feedPanel.firstChild);
        }
        slot.innerHTML = html;
        try {
          bindCreatePostComposer(UI);
        } catch (bindErr) {
          console.error("create post composer bind failed", bindErr);
        }
        scrollFeedToTop(false);
        var topbarTitle = document.querySelector(".app-topbar-title");
        if (topbarTitle && initialTopbarTitleHtml) topbarTitle.innerHTML = initialTopbarTitleHtml;
        if (initialDocumentTitle) document.title = initialDocumentTitle;
        if (window.history && window.history.pushState) {
          window.history.pushState({ panel: "feed-create" }, "", "/home");
        }
      } catch (e) {
        if (!isCurrentView("feed-create", viewNonce)) return;
        console.error("openCreatePanel failed", e);
        slot.innerHTML =
          '<section class="card app-card"><div class="card-accent" role="presentation"></div><p class="sub" role="alert"></p></section>';
        var msgEl = slot.querySelector("p[role='alert']");
        if (msgEl) msgEl.textContent = e && e.rateMsg ? e.rateMsg : UI.create_post_load_failed;
      }
    }

    function showHomeFeedPanel() {
      if (!feedPanel) return;
      currentProfileHandle = "";
      currentFeedPage = 0;
      var viewNonce = beginView("feed");
      profilePanel.hidden = true;
      if (createPanel) createPanel.hidden = true;
      feedPanel.hidden = false;
      removeJurorSummonSlot();
      removePrivateProfileCardSlot();
      renderFeedShell();
      ensureFeedScrollTopButton();
      bindFeedControls();
      pendingFeedScrollToTop = true;
      scrollFeedToTop(false);
      loadFeedDay(0, viewNonce);
      var existingCreate = document.getElementById("app-feed-create-slot");
      if (existingCreate) existingCreate.remove();
      var existingSearch = document.getElementById("app-feed-search-add-slot");
      if (existingSearch) existingSearch.remove();
      setRailSearchActive(false);
      var topbarTitle = document.querySelector(".app-topbar-title");
      if (topbarTitle && initialTopbarTitleHtml) topbarTitle.innerHTML = initialTopbarTitleHtml;
      if (initialDocumentTitle) document.title = initialDocumentTitle;
    }

    var avatarBtn = document.querySelector(".app-topbar-avatar-btn");
    if (avatarBtn) {
      avatarBtn.addEventListener("click", function (e) {
        if (avatarBtn.tagName === "A") e.preventDefault();
        if (!needsProfile && selfPrivateHandle) {
          openPrivateProfileCard(selfPrivateHandle, true);
          return;
        }
        openProfilePanel();
      });
    }

    document.addEventListener("app:avatar-redraw-complete", function (e) {
      var detail = e && e.detail ? e.detail : {};
      var path = detail.avatar_path ? String(detail.avatar_path) : "";
      if (!path) return;
      var src = path.charAt(0) === "/" || /^https?:\/\//i.test(path) ? path : "/" + path;
      if (src.indexOf("?") === -1) src += "?v=" + String(Date.now());
      var img = document.querySelector(
        ".app-topbar-avatar-btn .guest-council-avatar-img, .app-topbar-avatar-static .guest-council-avatar-img"
      );
      if (!img) return;
      img.removeAttribute("data-avatar-fallback");
      img.src = src;
    });

    var createBtn = document.getElementById("app-topbar-create-btn");
    if (createBtn && createBtn.tagName === "BUTTON") {
      createBtn.addEventListener("click", function (e) {
        e.preventDefault();
        e.stopPropagation();
        openCreatePanel();
      });
    }
    var runJobsBtn = document.getElementById("app-topbar-run-jobs-btn");
    if (runJobsBtn && runJobsBtn.tagName === "BUTTON") {
      runJobsBtn.addEventListener("click", async function (e) {
        e.preventDefault();
        if (runJobsBtn.disabled) return;
        ensureRightPanelNotificationHost();
        runJobsBtn.disabled = true;
        var originalLabel = runJobsBtn.textContent || "Run Jobs";
        runJobsBtn.textContent = UI.run_jobs_running || "Running...";
        try {
          var r = await fetch("/jobs/run-now", {
            method: "POST",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
          });
          if (!r.ok) throw new Error("run_failed");
          var data = await r.json().catch(function () {
            return {};
          });
          var s = data && data.summary ? data.summary : {};
          var claimed = Number(s.claimed || 0);
          var completed = Number(s.completed || 0);
          var msg;
          if (claimed === 0 && completed === 0) {
            msg = UI.run_jobs_idle || "Jobs tick finished — nothing was pending.";
          } else {
            msg =
              (UI.run_jobs_done || "Jobs run complete") +
              " — claimed " +
              String(claimed) +
              ", completed " +
              String(completed) +
              (Number(s.failed_retry || 0) > 0
                ? ", retry " + String(Number(s.failed_retry || 0))
                : "") +
              (Number(s.judicial_verdicts_resolved || 0) > 0
                ? ", verdicts " + String(Number(s.judicial_verdicts_resolved || 0))
                : "");
          }
          enqueueRightPanelNotification(msg, 6000);
        } catch (_) {
          enqueueRightPanelNotification(UI.run_jobs_failed || "Could not run jobs.", 3200);
        } finally {
          runJobsBtn.disabled = false;
          runJobsBtn.textContent = originalLabel;
        }
      });
    }
    async function openSearchAddPanel() {
      if (!feedPanel) return;
      var viewNonce = beginView("feed-search-add");
      profilePanel.hidden = true;
      if (createPanel) createPanel.hidden = true;
      feedPanel.hidden = false;
      removeJurorSummonSlot();
      renderFeedShell();
      bindFeedControls();
      loadFeedDay(currentFeedPage, viewNonce);
      var existingCreate = document.getElementById("app-feed-create-slot");
      if (existingCreate) existingCreate.remove();
      var existingSearch = document.getElementById("app-feed-search-add-slot");
      if (existingSearch) existingSearch.remove();
      setRailSearchActive(true);
      var loadingSlot = document.createElement("div");
      loadingSlot.id = "app-feed-search-add-slot";
      loadingSlot.className = "app-feed-search-add-slot";
      loadingSlot.innerHTML = panelCardLoaderHtml("home-search-add");
      feedPanel.insertBefore(loadingSlot, feedPanel.firstChild);
      ensureRightPanelNotificationHost();
      try {
        var r = await fetch(SEARCH_ADD_CARD_URL, {
          credentials: "same-origin",
          headers: { Accept: "text/html" },
        });
        if (r.status === 429) {
          var d429 = await r.json().catch(function () {
            return {};
          });
          throw { rateMsg: d429.error || UI.card_rate_limited };
        }
        if (!r.ok) throw new Error("load_failed");
        if (!isCurrentView("feed-search-add", viewNonce)) return;
        var html = await r.text();
        if (!isCurrentView("feed-search-add", viewNonce)) return;
        var slot = document.getElementById("app-feed-search-add-slot");
        if (!slot) {
          slot = document.createElement("div");
          slot.id = "app-feed-search-add-slot";
          slot.className = "app-feed-search-add-slot";
          feedPanel.insertBefore(slot, feedPanel.firstChild);
        }
        slot.innerHTML = html;
        bindSearchAddCard(UI);
        focusRailSearchInput();
        var topbarTitle = document.querySelector(".app-topbar-title");
        if (topbarTitle && initialTopbarTitleHtml) topbarTitle.innerHTML = initialTopbarTitleHtml;
        if (initialDocumentTitle) document.title = initialDocumentTitle;
        if (window.history && window.history.pushState) {
          window.history.pushState({ panel: "feed-search-add" }, "", "/home");
        }
      } catch (err) {
        setRailSearchActive(false);
        enqueueRightPanelNotification(
          (err && err.rateMsg) || UI.search_add_load_failed || "Could not load search/add card.",
          3000
        );
      }
    }

    bindRailSearchClose();

    var searchAddBtn = document.getElementById("app-rail-search-create-btn");
    if (searchAddBtn && searchAddBtn.tagName === "BUTTON") {
      searchAddBtn.addEventListener("click", async function (e) {
        e.preventDefault();
        e.stopPropagation();
        if (!feedPanel) return;
        await openSearchAddPanel();
      });
    }

    document.addEventListener("click", function (e) {
      var judicialCaseLink = e.target && e.target.closest ? e.target.closest('a[href^="/home/judicial/post/"]') : null;
      if (judicialCaseLink) {
        var hrefRaw = (judicialCaseLink.getAttribute("href") || "").trim();
        if (hrefRaw && hrefRaw.indexOf("return_to=") === -1) {
          var sep = hrefRaw.indexOf("?") >= 0 ? "&" : "?";
          var returnTo = window.location.pathname + window.location.search + window.location.hash;
          judicialCaseLink.setAttribute("href", hrefRaw + sep + "return_to=" + encodeURIComponent(returnTo));
        }
      }
      var privateMessageBtn =
        e.target && e.target.closest
          ? e.target.closest("button[data-profile-private-message]")
          : null;
      if (privateMessageBtn) {
        e.preventDefault();
        e.stopPropagation();
        var msgHandle = normalizeHandleCandidate(
          privateMessageBtn.getAttribute("data-profile-handle") || ""
        );
        if (msgHandle) {
          openCreatePanel({ privateMessageHandle: msgHandle });
        }
        return;
      }
      var publicShareBtn =
        e.target && e.target.closest
          ? e.target.closest("button.post-action-btn[data-public-link], button.public-post-share-btn")
          : null;
      if (publicShareBtn) {
        e.preventDefault();
        e.stopPropagation();
        var rawLink = (publicShareBtn.getAttribute("data-public-link") || "").trim();
        if (!rawLink) return;
        var fullLink = "";
        try {
          fullLink = new URL(rawLink, window.location.origin).toString();
        } catch (_) {
          fullLink = rawLink;
        }
        var copyDone = function () {
          publicShareBtn.setAttribute("title", "Public link copied");
          enqueueRightPanelNotification("URL copied to clipboard", 3000);
          window.setTimeout(function () {
            publicShareBtn.setAttribute("title", "Copy public link");
          }, 1200);
        };
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(fullLink).then(copyDone).catch(function () {});
        } else {
          var ta = document.createElement("textarea");
          ta.value = fullLink;
          ta.setAttribute("readonly", "readonly");
          ta.style.position = "fixed";
          ta.style.opacity = "0";
          document.body.appendChild(ta);
          ta.select();
          try {
            if (document.execCommand("copy")) copyDone();
          } catch (_) {}
          document.body.removeChild(ta);
        }
        return;
      }
      var profileHandleLink =
        e.target && e.target.closest
          ? e.target.closest("a.profile-handle-link, a.profile-civic-standing-community-link")
          : null;
      if (profileHandleLink) {
        var handleHref = (profileHandleLink.getAttribute("href") || "").trim();
        var handleMatch = handleHref.match(/^\/home\/profile\/([^/?#]+)$/);
        if (handleMatch && handleMatch[1]) {
          e.preventDefault();
          openPrivateProfileCard(decodeURIComponent(handleMatch[1]), true);
          return;
        }
      }
      var profileLink = e.target && e.target.closest ? e.target.closest("a.feed-profile-link") : null;
      if (profileLink) {
        var handleFromData = (profileLink.getAttribute("data-profile-handle") || "").trim();
        var href = (profileLink.getAttribute("href") || "").trim();
        var m2 = href.match(/^\/home\/profile\/([^/?#]+)$/);
        var handle = handleFromData || (m2 ? decodeURIComponent(m2[1]) : "");
        handle = normalizeHandleCandidate(handle);
        if (handle) {
          e.preventDefault();
          openPrivateProfileCard(handle, true);
          return;
        }
      }
      var link = e.target && e.target.closest ? e.target.closest("a.app-rail-slot--filled") : null;
      if (!link) return;
      var href = (link.getAttribute("href") || "").trim();
      var m = href.match(/^\/home\/profile\/([^/?#]+)$/);
      if (!m) return;
      e.preventDefault();
      scrollFeedToTop();
      openPrivateProfileCard(decodeURIComponent(m[1]), true);
    });

    window.addEventListener("popstate", function () {
      var path = (window.location && window.location.pathname) || "";
      var m = path.match(/^\/home\/profile\/([^/?#]+)$/);
      if (m && m[1]) {
        openPrivateProfileCard(decodeURIComponent(m[1]), false);
        return;
      }
      if (path === "/home" || path === "/home/") {
        showHomeFeedPanel();
      }
    });

    window.addEventListener("pageshow", function (e) {
      var path = (window.location && window.location.pathname) || "";
      if (path !== "/home" && path !== "/home/") return;
      var nav =
        performance.getEntriesByType && performance.getEntriesByType("navigation")[0];
      if (e.persisted || (nav && nav.type === "back_forward")) {
        pendingFeedScrollToTop = true;
        scrollFeedToTop(false);
      }
    });

    showInactivityRefundNoticeFromQuery();

    if (needsConsent) {
      if (feedPanel) feedPanel.hidden = true;
      syncFeedScrollTopButton();
      var consentPanel = document.getElementById("app-home-consent-panel");
      var consentLoading = document.getElementById("app-home-consent-loading");
      var consentLoadingMsg = document.getElementById("app-home-consent-loading-msg");
      var consentMount = document.getElementById("app-home-consent-mount");
      if (consentPanel) consentPanel.hidden = false;
      fetchHtmlCard(CONSENT_CARD_URL)
        .then(function (html) {
          if (consentMount) consentMount.innerHTML = html;
          if (consentLoading) consentLoading.hidden = true;
          bindTermsConsentForm(UI);
        })
        .catch(function (e) {
          if (consentLoadingMsg) {
            consentLoadingMsg.textContent =
              e && e.rateMsg ? e.rateMsg : UI.terms_step_failed || "Could not load consent step.";
            consentLoadingMsg.hidden = false;
          }
        });
      return;
    }

    if (!needsProfile) {
      stripStaleOnboardingParams();
      if (feedPanel) {
        var initialPath = (window.location && window.location.pathname) || "";
        var initialProfileMatch = initialPath.match(/^\/home\/profile\/([^/?#]+)$/);
        if (initialProfileMatch && initialProfileMatch[1]) {
          openPrivateProfileCard(decodeURIComponent(initialProfileMatch[1]), false);
        } else {
          showHomeFeedPanel();
        }
        try {
          var noticeParams = new URLSearchParams(window.location.search || "");
          if (noticeParams.get("notice") === "constitutional_amendment_filed") {
            ensureRightPanelNotificationHost();
            enqueueRightPanelNotification(
              UI.constitutional_amendment_filed_notice ||
                "Constitutional amendment filed",
              4000
            );
          }
          if (noticeParams.get("notice") === "community_profile_amendment_filed") {
            ensureRightPanelNotificationHost();
            enqueueRightPanelNotification(
              UI.community_profile_amendment_filed_notice ||
                "Community profile change filed",
              4000
            );
          }
          if (noticeParams.get("notice") === "judicial_case_filed") {
            ensureRightPanelNotificationHost();
            enqueueRightPanelNotification(
              UI.judicial_case_filed_notice || "Judicial case filed",
              4000
            );
            noticeParams.delete("notice");
            var q = noticeParams.toString();
            var cleanUrl =
              window.location.pathname + (q ? "?" + q : "") + (window.location.hash || "");
            if (window.history && window.history.replaceState) {
              window.history.replaceState(window.history.state || {}, "", cleanUrl);
            }
          } else if (noticeParams.get("notice") === "emergency_contest_opened") {
            ensureRightPanelNotificationHost();
            enqueueRightPanelNotification(
              UI.emergency_contest_opened_notice || "Emergency contest opened",
              4000
            );
            noticeParams.delete("notice");
            var qContest = noticeParams.toString();
            var cleanUrlContest =
              window.location.pathname +
              (qContest ? "?" + qContest : "") +
              (window.location.hash || "");
            if (window.history && window.history.replaceState) {
              window.history.replaceState(
                window.history.state || {},
                "",
                cleanUrlContest
              );
            }
          } else if (noticeParams.get("notice") === "judicial_appeal_filed") {
            ensureRightPanelNotificationHost();
            enqueueRightPanelNotification(
              UI.judicial_appeal_filed_notice || "Appeal filed",
              4000
            );
            noticeParams.delete("notice");
            var qAppeal = noticeParams.toString();
            var cleanUrlAppeal =
              window.location.pathname +
              (qAppeal ? "?" + qAppeal : "") +
              (window.location.hash || "");
            if (window.history && window.history.replaceState) {
              window.history.replaceState(
                window.history.state || {},
                "",
                cleanUrlAppeal
              );
            }
          }
        } catch (_) {}
      }
      return;
    }

    // Onboarding-required users continue to the profile step after consent and face capture.
    if (feedPanel) feedPanel.hidden = true;
    syncFeedScrollTopButton();
    profilePanel.hidden = false;

    function fetchHtmlCard(url) {
      return fetch(url, { credentials: "same-origin", headers: { Accept: "text/html" } }).then(
        function (r) {
          if (r.status === 429) {
            return r.json().then(function (d) {
              var err = new Error("rate_limited");
              err.rateMsg = d.error || UI.card_rate_limited;
              throw err;
            });
          }
          if (!r.ok) throw new Error("load_failed");
          return r.text();
        }
      );
    }

    function showProfileAfterTerms() {
      var step1 = document.getElementById("app-home-profile-step1");
      if (step1) step1.hidden = false;
      loadingEl.hidden = false;
      mountEl.innerHTML = "";
      if (loadingMsg) {
        loadingMsg.textContent = "";
        loadingMsg.hidden = true;
      }
      fetchHtmlCard(CARD_URL)
        .then(function (html) {
          mountEl.innerHTML = html;
          loadingEl.hidden = true;
          bindProfileForm(UI);
          startProfileCountryDetection();
        })
        .catch(function (e) {
          if (loadingMsg) {
            loadingMsg.textContent = e && e.rateMsg ? e.rateMsg : UI.profile_load_failed;
            loadingMsg.hidden = false;
          }
        });
    }

    showProfileAfterTerms();

    window.showHomeFeedPanel = showHomeFeedPanel;
    window.showHomeToast = function (message, visibleMs) {
      enqueueRightPanelNotification(message, visibleMs || 3200);
    };
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", run);
  } else {
    run();
  }
})();

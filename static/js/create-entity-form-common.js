/**
 * Shared bindings for create-entity and profile entity-edit forms:
 * rich description editor, Google Places search, co-owner handle input.
 */
(function (global) {
  "use strict";

  var DEFAULT_DESC_MAX = 4000;
  var PLACE_CACHE_KEY_PREFIX = "googlePlaceSearchCacheV1:";
  var PLACE_CACHE_MAX_ITEMS = 24;
  var ADMIN_PREFIX = "Add: ";

  function defaultEscapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function markupLength(html) {
    return String(html || "").replace(/\r\n/g, "\n").length;
  }

  function stripOwnerHandlesPrefix(raw) {
    return String(raw || "")
      .replace(/^add:\s*/i, "")
      .replace(/^for:\s*/i, "")
      .replace(/^owners:\s*/i, "")
      .trim();
  }

  function setPlaceBadge(badge, msg, mode) {
    if (!badge) return;
    badge.classList.remove(
      "handle-availability-badge--checking",
      "handle-availability-badge--error",
      "handle-availability-badge--success"
    );
    if (!msg) {
      badge.hidden = true;
      badge.textContent = "";
      return;
    }
    badge.hidden = false;
    badge.textContent = msg;
    if (mode === "checking") badge.classList.add("handle-availability-badge--checking");
    else if (mode === "error") badge.classList.add("handle-availability-badge--error");
    else if (mode === "success") badge.classList.add("handle-availability-badge--success");
  }

  function readPlaceCache(query) {
    var q = String(query || "").trim().toLowerCase();
    if (!q) return null;
    try {
      var raw = sessionStorage.getItem(PLACE_CACHE_KEY_PREFIX + q);
      if (!raw) return null;
      var parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : null;
    } catch (_e) {
      return null;
    }
  }

  function writePlaceCache(query, rows) {
    var q = String(query || "").trim().toLowerCase();
    if (!q) return;
    try {
      sessionStorage.setItem(PLACE_CACHE_KEY_PREFIX + q, JSON.stringify(rows || []));
      var keysRaw = sessionStorage.getItem(PLACE_CACHE_KEY_PREFIX + "_keys");
      var keys = [];
      if (keysRaw) {
        var parsed = JSON.parse(keysRaw);
        if (Array.isArray(parsed)) keys = parsed;
      }
      keys = keys.filter(function (k) {
        return k !== q;
      });
      keys.push(q);
      while (keys.length > PLACE_CACHE_MAX_ITEMS) {
        var drop = keys.shift();
        if (drop) sessionStorage.removeItem(PLACE_CACHE_KEY_PREFIX + drop);
      }
      sessionStorage.setItem(PLACE_CACHE_KEY_PREFIX + "_keys", JSON.stringify(keys));
    } catch (_e) {}
  }

  function syncRichDescription(descriptionInput, descriptionEditor) {
    if (!descriptionInput || !descriptionEditor) return;
    descriptionInput.value = String(descriptionEditor.innerHTML || "").trim();
  }

  function bindRichDescription(opts) {
    var descriptionInput = opts.descriptionInput;
    var descriptionEditor = opts.descriptionEditor;
    var descriptionToolbar = opts.descriptionToolbar;
    var descriptionCharHint = opts.descriptionCharHint;
    var descriptionInitial = opts.descriptionInitial;
    var maxChars = opts.maxChars || DEFAULT_DESC_MAX;
    if (!descriptionInput || !descriptionEditor) return;

    function editorMarkupLength() {
      return markupLength(descriptionEditor.innerHTML);
    }

    function syncField() {
      descriptionInput.value = String(descriptionEditor.innerHTML || "").trim();
    }

    function updateHint() {
      if (!descriptionCharHint) return;
      var used = Math.max(0, Math.min(maxChars, editorMarkupLength()));
      descriptionCharHint.textContent = String(used) + "/" + String(maxChars);
      descriptionCharHint.classList.toggle("is-limit", used >= maxChars);
    }

    function enforceLimit() {
      if (editorMarkupLength() <= maxChars) return;
      try {
        document.execCommand("undo", false, null);
      } catch (_e) {}
      if (editorMarkupLength() > maxChars) {
        descriptionEditor.textContent = String(descriptionEditor.innerText || "")
          .replace(/\r\n/g, "\n")
          .slice(0, maxChars);
      }
    }

    if (descriptionInitial && descriptionInitial.value) {
      descriptionEditor.innerHTML = descriptionInitial.value;
    }
    syncField();
    updateHint();

    if (descriptionToolbar) {
      var colorBtns = descriptionToolbar.querySelectorAll(".create-post-editor-color-btn");
      colorBtns.forEach(function (btn) {
        btn.textContent = "";
        var swatch = (btn.getAttribute("data-value") || "").trim();
        if (swatch) btn.style.setProperty("--swatch", swatch);
      });
      descriptionToolbar.addEventListener("click", function (e) {
        var btn = e.target && e.target.closest ? e.target.closest("[data-cmd]") : null;
        if (!btn) return;
        e.preventDefault();
        var cmd = btn.getAttribute("data-cmd");
        if (!cmd) return;
        descriptionEditor.focus();
        var value = btn.getAttribute("data-value");
        document.execCommand(cmd, false, value != null ? value : null);
        enforceLimit();
        syncField();
        updateHint();
      });
    }

    descriptionEditor.addEventListener("input", function () {
      enforceLimit();
      syncField();
      updateHint();
    });
    descriptionEditor.addEventListener("paste", function () {
      window.setTimeout(function () {
        enforceLimit();
        syncField();
        updateHint();
      }, 0);
    });
  }

  function bindOwnerHandles(input) {
    if (!input || input.getAttribute("data-owner-handles-bound") === "1") return;
    input.setAttribute("data-owner-handles-bound", "1");

    function adminPrefixLen() {
      var v = String(input.value || "");
      var m = v.match(/^add:\s*/i);
      return m ? m[0].length : 0;
    }

    function extractAdminHandles(raw) {
      var s = stripOwnerHandlesPrefix(raw);
      var matches = s.match(/@[A-Za-z0-9_]+/g) || [];
      return matches.map(function (h) {
        return h.toLowerCase();
      });
    }

    function normalizeAdminHandles(raw) {
      var handles = extractAdminHandles(raw);
      var seen = {};
      var out = [];
      handles.forEach(function (h) {
        if (!seen[h]) {
          seen[h] = true;
          out.push(h);
        }
      });
      if (out.length === 0) return "";
      return ADMIN_PREFIX + out.join(", ");
    }

    function keepAdminCaretAfterPrefix() {
      var minPos = adminPrefixLen();
      if (minPos === 0) return;
      var start = input.selectionStart || 0;
      var end = input.selectionEnd || 0;
      if (start < minPos || end < minPos) {
        input.setSelectionRange(minPos, minPos);
      }
    }

    input.addEventListener("click", keepAdminCaretAfterPrefix);
    input.addEventListener("focus", keepAdminCaretAfterPrefix);
    input.addEventListener("keydown", function (e) {
      var minPos = adminPrefixLen();
      var start = input.selectionStart || 0;
      var end = input.selectionEnd || 0;
      var v = input.value;
      if (minPos === 0) {
        if (e.key === "@" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
          e.preventDefault();
          input.value = ADMIN_PREFIX + "@";
          input.setSelectionRange(ADMIN_PREFIX.length + 1, ADMIN_PREFIX.length + 1);
          return;
        }
        if (e.key === " " && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
          e.preventDefault();
          input.value = ADMIN_PREFIX + "@";
          input.setSelectionRange(ADMIN_PREFIX.length + 1, ADMIN_PREFIX.length + 1);
          return;
        }
      }
      if (minPos > 0) {
        if (
          (e.key === "Backspace" && start <= minPos) ||
          (e.key === "ArrowLeft" && start <= minPos && end <= minPos)
        ) {
          e.preventDefault();
          input.setSelectionRange(minPos, minPos);
          return;
        }
        if (e.key === "Delete" && start < minPos) {
          e.preventDefault();
          input.setSelectionRange(minPos, minPos);
          return;
        }
      }
      if (e.key === " " && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
        if (minPos === 0) return;
        e.preventDefault();
        var prevCh = start > 0 ? v.slice(start - 1, start) : "";
        if (prevCh === "@") return;
        var insert = prevCh === " " || prevCh === "\t" ? "@" : " @";
        input.value = v.slice(0, start) + insert + v.slice(end);
        var nextPos = start + insert.length;
        input.setSelectionRange(nextPos, nextPos);
        return;
      }
      if (e.key === "@" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
        if (minPos > 0 && start > minPos) {
          var prev = v.slice(start - 1, start);
          if (prev !== " " && prev !== "\t") {
            e.preventDefault();
            var insertAt = " @";
            input.value = v.slice(0, start) + insertAt + v.slice(end);
            var pos = start + insertAt.length;
            input.setSelectionRange(pos, pos);
          }
        }
      }
    });
    input.addEventListener("input", keepAdminCaretAfterPrefix);
    input.addEventListener("blur", function () {
      var next = normalizeAdminHandles(input.value);
      if (next !== input.value) input.value = next;
      keepAdminCaretAfterPrefix();
    });
  }

  function bindGooglePlaceSearch(opts) {
    var locInput = opts.locInput;
    var placeQueryInput = opts.placeQueryInput;
    var placeResults = opts.placeResults;
    var placeBadge = opts.placeBadge;
    var placeIdInput = opts.placeIdInput;
    var UI = opts.UI || {};
    var escapeHtml = opts.escapeHtml || defaultEscapeHtml;
    var buildHeaders =
      typeof opts.buildHeaders === "function"
        ? opts.buildHeaders
        : function (extra) {
            var h = { Accept: "application/json" };
            if (extra) {
              for (var k in extra) {
                if (Object.prototype.hasOwnProperty.call(extra, k)) h[k] = extra[k];
              }
            }
            return h;
          };
    var useCache = opts.useCache !== false;
    var debounceMs = opts.debounceMs || 280;
    var clearLocationOnQueryChange = !!opts.clearLocationOnQueryChange;
    var initialShowReadyIfSet = !!opts.initialShowReadyIfSet;
    var onVerified = typeof opts.onVerified === "function" ? opts.onVerified : function () {};
    var onVerifyError = typeof opts.onVerifyError === "function" ? opts.onVerifyError : function () {};
    var onQueryChange = typeof opts.onQueryChange === "function" ? opts.onQueryChange : function () {};

    if (!locInput || !placeQueryInput) return;

    var placeSearchState = { token: 0 };
    var placeSearchTimer = null;

    function renderResults(items) {
      if (!placeResults) return;
      if (!items || !items.length) {
        placeResults.hidden = true;
        placeResults.innerHTML = "";
        return;
      }
      placeResults.hidden = false;
      placeResults.innerHTML = items
        .map(function (it) {
          var pid = String((it && it.place_id) || "").trim();
          var main = String((it && it.main_text) || "").trim();
          var secondary = String((it && it.secondary_text) || "").trim();
          if (!pid || !main) return "";
          return (
            '<li><button type="button" class="create-entity-place-result-btn" data-place-id="' +
            escapeHtml(pid) +
            '" data-place-label="' +
            escapeHtml(main + (secondary ? ", " + secondary : "")) +
            '"><span class="create-entity-place-main">' +
            escapeHtml(main) +
            "</span>" +
            (secondary
              ? '<span class="create-entity-place-secondary">' + escapeHtml(secondary) + "</span>"
              : "") +
            "</button></li>"
          );
        })
        .join("");
    }

    function confirmPlace(pid) {
      if (!pid) return;
      setPlaceBadge(placeBadge, UI.create_entity_place_verifying || "Verifying address...", "checking");
      fetch("/me/google-place-location", {
        method: "POST",
        credentials: "same-origin",
        headers: buildHeaders({ "Content-Type": "application/json", Accept: "application/json" }),
        body: JSON.stringify({ place_id: pid, update_human_entity: false }),
      })
        .then(function (r) {
          if (r.status === 429) {
            return r.json().then(function (d) {
              throw new Error((d && d.error) || UI.card_rate_limited || "Rate limited.");
            });
          }
          if (!r.ok) {
            return r.json().then(function (d) {
              throw new Error(
                (d && d.error) ||
                  UI.create_entity_place_search_failed ||
                  UI.request_failed ||
                  "Request failed."
              );
            });
          }
          return r.json();
        })
        .then(function (data) {
          locInput.value = String((data && data.location_id) || "").trim();
          setPlaceBadge(placeBadge, UI.handle_ready || "Ready", "success");
          onVerified(data);
        })
        .catch(function (err) {
          setPlaceBadge(
            placeBadge,
            (err && err.message) ||
              UI.create_entity_place_search_failed ||
              UI.request_failed ||
              "Address could not be verified.",
            "error"
          );
          if (clearLocationOnQueryChange && locInput) locInput.value = "";
          onVerifyError(err);
        });
    }

    function runSearch() {
      var q = String(placeQueryInput.value || "").trim();
      if (q.length < 5) {
        renderResults([]);
        setPlaceBadge(
          placeBadge,
          q.length > 0 ? UI.create_entity_place_query_min || "Address search starts at 5 characters." : "",
          q.length > 0 ? "error" : ""
        );
        return;
      }
      var token = placeSearchState.token + 1;
      placeSearchState.token = token;
      setPlaceBadge(placeBadge, UI.create_entity_place_searching || "Searching addresses...", "checking");
      if (useCache) {
        var cached = readPlaceCache(q);
        if (cached) {
          renderResults(cached);
          setPlaceBadge(placeBadge, "", "");
          return;
        }
      }
      fetch("/me/google-place-search?q=" + encodeURIComponent(q), {
        credentials: "same-origin",
        headers: buildHeaders({ Accept: "application/json" }),
      })
        .then(function (r) {
          if (r.status === 429) {
            return r.json().then(function (d) {
              throw new Error((d && d.error) || UI.card_rate_limited || "Rate limited.");
            });
          }
          if (!r.ok) {
            return r
              .json()
              .catch(function () {
                return {};
              })
              .then(function (d) {
                throw new Error(
                  (d && d.error) ||
                    UI.create_entity_place_search_failed ||
                    UI.request_failed ||
                    "Search failed."
                );
              });
          }
          return r.json();
        })
        .then(function (data) {
          if (placeSearchState.token !== token) return;
          var list = Array.isArray(data) ? data : data && data.results ? data.results : [];
          renderResults(list);
          if (useCache) writePlaceCache(q, list);
          setPlaceBadge(placeBadge, "", "");
        })
        .catch(function (err) {
          if (placeSearchState.token !== token) return;
          renderResults([]);
          setPlaceBadge(
            placeBadge,
            (err && err.message) ||
              UI.create_entity_place_search_failed ||
              UI.request_failed ||
              "Search failed.",
            "error"
          );
        });
    }

    placeQueryInput.addEventListener("input", function () {
      if (clearLocationOnQueryChange) {
        if (placeIdInput) placeIdInput.value = "";
        if (locInput) locInput.value = "";
      }
      onQueryChange();
      if (placeSearchTimer) window.clearTimeout(placeSearchTimer);
      placeSearchTimer = window.setTimeout(runSearch, debounceMs);
    });

    if (placeResults) {
      placeResults.addEventListener("click", function (e) {
        var btn = e.target && e.target.closest ? e.target.closest(".create-entity-place-result-btn") : null;
        if (!btn) return;
        var pid = String(btn.getAttribute("data-place-id") || "").trim();
        var label = String(btn.getAttribute("data-place-label") || "").trim();
        if (placeIdInput) placeIdInput.value = pid;
        if (label) placeQueryInput.value = label;
        renderResults([]);
        confirmPlace(pid);
      });
    }

    if (
      initialShowReadyIfSet &&
      String(locInput.value || "").trim() &&
      String(placeQueryInput.value || "").trim()
    ) {
      setPlaceBadge(placeBadge, UI.handle_ready || "Ready", "success");
    }
  }

  global.CreateEntityFormCommon = {
    DEFAULT_DESC_MAX: DEFAULT_DESC_MAX,
    ADMIN_PREFIX: ADMIN_PREFIX,
    markupLength: markupLength,
    stripOwnerHandlesPrefix: stripOwnerHandlesPrefix,
    syncRichDescription: syncRichDescription,
    bindRichDescription: bindRichDescription,
    bindOwnerHandles: bindOwnerHandles,
    bindGooglePlaceSearch: bindGooglePlaceSearch,
  };
})(typeof window !== "undefined" ? window : globalThis);

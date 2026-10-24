/**
 * Shared handle input behavior for social_entity handle collection.
 */
(function () {
  "use strict";

  function normalizeHandleCandidate(raw) {
    var s = String(raw || "").trim().toLowerCase();
    s = s.replace(/^@+/, "");
    return s;
  }

  function handleHasMinLength(raw) {
    return normalizeHandleCandidate(raw).length >= 3;
  }

  function formatSingleHandleInput(raw, keepPrefixWhenEmpty) {
    var token = String(raw || "").trim().split(/[\s,]+/, 1)[0] || "";
    var normalized = normalizeHandleCandidate(token);
    if (!normalized) return keepPrefixWhenEmpty ? "@" : "";
    return "@" + normalized;
  }

  function applyHandleInputDefaults(inputEl, options) {
    if (!inputEl) return;
    var opts = options || {};
    var keepPrefixOnFocus = opts.keepPrefixOnFocus !== false;
    var useDefaultPlaceholder = opts.useDefaultPlaceholder !== false;
    if (useDefaultPlaceholder) {
      inputEl.placeholder = inputEl.placeholder || "@handle";
    }
    var moveCaretToEnd = function () {
      if (!inputEl.setSelectionRange) return;
      var pos = String(inputEl.value || "").length;
      inputEl.setSelectionRange(pos, pos);
    };
    inputEl.addEventListener("focus", function () {
      var next = formatSingleHandleInput(inputEl.value, keepPrefixOnFocus);
      if (next !== inputEl.value) inputEl.value = next;
      moveCaretToEnd();
    });
    inputEl.addEventListener("blur", function () {
      var next = formatSingleHandleInput(inputEl.value, false);
      if (next !== inputEl.value) inputEl.value = next;
    });
    inputEl.addEventListener("input", function () {
      var next = formatSingleHandleInput(inputEl.value, keepPrefixOnFocus);
      if (next !== inputEl.value) {
        inputEl.value = next;
        moveCaretToEnd();
      }
    });
  }

  window.AppHandleInput = {
    normalizeHandleCandidate: normalizeHandleCandidate,
    handleHasMinLength: handleHasMinLength,
    formatSingleHandleInput: formatSingleHandleInput,
    applyHandleInputDefaults: applyHandleInputDefaults,
  };
})();

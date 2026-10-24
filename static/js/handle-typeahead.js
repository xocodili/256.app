/**
 * Shared @-handle typeahead for audience fields, mentions, and global entity search.
 *
 * Modes:
 * - global: all alive entities (`GET /social-entities/search`)
 * - audience: connections + system scopes (`POST /posts/audience-handle-candidates`)
 * - mention: audience-scoped entities (`POST /posts/audience-mention-candidates`)
 */
(function (global) {
  "use strict";

  var MIN_PREFIX = 3;
  var DEBOUNCE_MS = 180;
  var SEARCH_ENTITIES_URL = "/social-entities/search";
  var AUDIENCE_HANDLES_URL = "/posts/audience-handle-candidates";
  var MENTION_CANDIDATES_URL = "/posts/audience-mention-candidates";

  function escapeHtml(s) {
    return String(s || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function debounce(fn, ms) {
    var t;
    return function () {
      var args = arguments;
      var self = this;
      clearTimeout(t);
      t = setTimeout(function () {
        fn.apply(self, args);
      }, ms);
    };
  }

  function prefixSearchable(raw) {
    var s = String(raw || "")
      .trim()
      .toLowerCase()
      .replace(/^@+/, "");
    var alnum = (s.match(/[a-z0-9]/g) || []).length;
    if (alnum < MIN_PREFIX) return "";
    return s.slice(0, 32);
  }

  function stripAt(handle) {
    return String(handle || "")
      .trim()
      .replace(/^@+/, "");
  }

  function atTokenAtCaretInput(input) {
    if (!input || input.readOnly || input.disabled) return null;
    var val = String(input.value || "");
    var pos = input.selectionStart == null ? val.length : input.selectionStart;
    var before = val.slice(0, pos);
    var at = before.lastIndexOf("@");
    if (at < 0) return null;
    var token = before.slice(at + 1);
    if (!token || /\s/.test(token)) return null;
    return { atIndex: at, prefix: token, endPos: pos };
  }

  function walkCharOffsetToNode(root, charIndex) {
    var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    var node;
    var count = 0;
    while ((node = walker.nextNode())) {
      var len = node.textContent.length;
      if (count + len >= charIndex) {
        return { node: node, offset: charIndex - count };
      }
      count += len;
    }
    return null;
  }

  function atTokenAtCaretEditor(editor) {
    if (!editor) return null;
    var sel = global.getSelection();
    if (!sel || !sel.rangeCount) return null;
    var range = sel.getRangeAt(0);
    if (!range.collapsed || !editor.contains(range.commonAncestorContainer)) return null;
    var r = document.createRange();
    r.setStart(editor, 0);
    r.setEnd(range.endContainer, range.endOffset);
    var textBefore = r.toString();
    var at = textBefore.lastIndexOf("@");
    if (at < 0) return null;
    var prefix = textBefore.slice(at + 1);
    if (!prefix || /\s/.test(prefix)) return null;
    return { atIndex: at, prefix: prefix, range: range };
  }

  function createListbox(anchorEl, className, ariaLabel) {
    var ul = document.createElement("ul");
    ul.className = className || "handle-typeahead-list search-add-results";
    ul.hidden = true;
    ul.setAttribute("role", "listbox");
    if (ariaLabel) ul.setAttribute("aria-label", ariaLabel);
    if (anchorEl && anchorEl.parentNode) {
      anchorEl.parentNode.insertBefore(ul, anchorEl.nextSibling);
    }
    return ul;
  }

  function createOverlayListbox(className, ariaLabel) {
    var ul = document.createElement("ul");
    ul.className = className || "handle-typeahead-overlay handle-typeahead-list";
    ul.hidden = true;
    ul.setAttribute("role", "listbox");
    if (ariaLabel) ul.setAttribute("aria-label", ariaLabel);
    document.body.appendChild(ul);
    return ul;
  }

  function caretClientRect(editor) {
    if (!editor) return null;
    var sel = global.getSelection();
    if (!sel || !sel.rangeCount) return null;
    var range = sel.getRangeAt(0);
    if (!range.collapsed || !editor.contains(range.commonAncestorContainer)) return null;
    var marker = range.cloneRange();
    marker.collapse(true);
    var rects = marker.getClientRects();
    if (rects && rects.length) {
      return rects[rects.length - 1];
    }
    var box = marker.getBoundingClientRect();
    if (box.width || box.height) return box;
    var editorBox = editor.getBoundingClientRect();
    return {
      left: editorBox.left + 12,
      right: editorBox.left + 12,
      top: editorBox.top + 12,
      bottom: editorBox.top + 12,
      width: 0,
      height: 0,
    };
  }

  var INPUT_CARET_MIRROR_PROPS = [
    "boxSizing",
    "width",
    "height",
    "overflowX",
    "overflowY",
    "borderTopWidth",
    "borderRightWidth",
    "borderBottomWidth",
    "borderLeftWidth",
    "paddingTop",
    "paddingRight",
    "paddingBottom",
    "paddingLeft",
    "fontStyle",
    "fontVariant",
    "fontWeight",
    "fontStretch",
    "fontSize",
    "lineHeight",
    "fontFamily",
    "textAlign",
    "textTransform",
    "textIndent",
    "letterSpacing",
    "wordSpacing",
    "whiteSpace",
  ];

  function caretClientRectForInput(input) {
    if (!input || input.readOnly || input.disabled) return null;
    var pos = input.selectionStart;
    if (pos == null) pos = String(input.value || "").length;
    var computed = global.getComputedStyle(input);
    var inputRect = input.getBoundingClientRect();
    var div = document.createElement("div");
    var style = div.style;
    style.position = "absolute";
    style.visibility = "hidden";
    style.top = inputRect.top + "px";
    style.left = inputRect.left + "px";
    INPUT_CARET_MIRROR_PROPS.forEach(function (prop) {
      style[prop] = computed[prop];
    });
    if (input.nodeName === "INPUT") {
      style.whiteSpace = "pre";
    }
    var val = String(input.value || "");
    div.textContent = val.substring(0, pos);
    var span = document.createElement("span");
    span.textContent = val.substring(pos) || "\u200b";
    div.appendChild(span);
    document.body.appendChild(div);
    var rect = span.getBoundingClientRect();
    document.body.removeChild(div);
    if (!rect.width && !rect.height) {
      var padL = parseFloat(computed.paddingLeft) || 0;
      var padT = parseFloat(computed.paddingTop) || 0;
      var lineH = parseFloat(computed.lineHeight) || 16;
      return {
        left: inputRect.left + padL,
        right: inputRect.left + padL,
        top: inputRect.top + padT,
        bottom: inputRect.top + padT + lineH,
        width: 0,
        height: 0,
      };
    }
    return rect;
  }

  function positionOverlayList(listEl, anchorEl, caretRect) {
    if (!listEl || !anchorEl || !caretRect) return;
    var gap = 4;
    var anchorBox = anchorEl.getBoundingClientRect();
    var maxW = Math.min(22 * 16, Math.max(11 * 16, anchorBox.width - 16));
    listEl.style.maxWidth = maxW + "px";
    var listH = listEl.offsetHeight || 0;
    var left = Math.max(8, Math.min(caretRect.left, global.innerWidth - maxW - 8));
    var top = caretRect.bottom + gap;
    if (top + listH > global.innerHeight - 8 && caretRect.top - listH - gap > 8) {
      top = caretRect.top - listH - gap;
    }
    listEl.style.left = left + "px";
    listEl.style.top = top + "px";
  }

  function formatShowAllLabel(template, count) {
    var t = String(template || "Show all ({count})");
    return t.replace(/\{count\}/g, String(count));
  }

  function renderOptions(listEl, items, renderItem) {
    if (!listEl) return;
    if (!items || !items.length) {
      listEl.innerHTML = "";
      listEl.hidden = true;
      return;
    }
    listEl.innerHTML = items.map(renderItem).join("");
    listEl.hidden = false;
  }

  function fetchGlobal(query) {
    return fetch(SEARCH_ENTITIES_URL + "?q=" + encodeURIComponent(query), {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    }).then(function (r) {
      if (!r.ok) throw new Error("search_failed");
      return r.json();
    });
  }

  function fetchAudience(prefix) {
    return fetch(AUDIENCE_HANDLES_URL, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify({ prefix: prefix }),
    }).then(function (r) {
      if (!r.ok) throw new Error("audience_failed");
      return r.json();
    });
  }

  function fetchMention(ctx, prefix) {
    var body = {
      entity_id: String(ctx.entityId || ""),
      audience_handles: String(ctx.audienceHandles || ""),
      prefix: prefix,
    };
    if (ctx.rootPostId) {
      body.root_post_id = String(ctx.rootPostId);
    }
    return fetch(MENTION_CANDIDATES_URL, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body),
    }).then(function (r) {
      if (!r.ok) throw new Error("mention_failed");
      return r.json();
    });
  }

  function defaultMentionOption(it) {
    var h = stripAt(it && it.handle);
    var label = String((it && it.label) || h).trim();
    var kind = String((it && it.kind) || "").trim();
    var type = String((it && it.type_key) || kind || "").trim();
    return (
      '<li role="option"><button type="button" class="handle-typeahead-option" data-handle="' +
      escapeHtml(h) +
      '">@' +
      escapeHtml(h) +
      (label && label !== h
        ? ' <span class="sub">' + escapeHtml(label) + "</span>"
        : "") +
      (type && type !== "scope"
        ? ' <span class="sub handle-typeahead-kind"> · ' + escapeHtml(type) + "</span>"
        : "") +
      "</button></li>"
    );
  }

  function defaultGlobalOption(it) {
    var handle = stripAt(it && it.handle);
    var showHandle = "@" + handle;
    var type = String((it && it.type_key) || "").trim();
    var label = String((it && it.label) || showHandle).trim();
    return (
      '<li class="search-add-result-item" role="option"><a class="feed-profile-link search-add-result-link handle-typeahead-option" href="/home/profile/' +
      encodeURIComponent(handle) +
      '" data-profile-handle="' +
      escapeHtml(handle) +
      '" data-handle="' +
      escapeHtml(handle) +
      '">' +
      escapeHtml(showHandle) +
      "</a>" +
      '<span class="sub search-add-result-sub">' +
      escapeHtml(label) +
      (type ? " · " + escapeHtml(type) : "") +
      "</span></li>"
    );
  }

  function bindInputTypeahead(options) {
    options = options || {};
    var input = options.input;
    if (!input) return { destroy: function () {} };

    var mode = options.mode || "global";
    var trigger = options.trigger || "at";

    if (mode === "mention" && trigger === "at" && options.overlay !== false) {
      return bindMentionOverlayTypeahead(
        Object.assign({}, options, {
          field: input,
          fieldType: "input",
          listClass:
            options.listClass ||
            "handle-typeahead-overlay handle-typeahead-list post-engagement-mention-suggestions",
          fallbackGlobal: options.fallbackGlobal === true,
        })
      );
    }
    var listEl =
      options.listEl ||
      createListbox(
        input,
        options.listClass || "handle-typeahead-list search-add-results",
        options.ariaLabel || "Handle suggestions"
      );
    var getContext = options.getContext || function () { return {}; };
    var onSelect = options.onSelect;
    var fetchToken = 0;
    var activeIndex = -1;

    function hide() {
      activeIndex = -1;
      listEl.innerHTML = "";
      listEl.hidden = true;
    }

    function insertAtToken(handle) {
      handle = stripAt(handle);
      if (!handle) return;
      if (trigger === "full") {
        if (typeof onSelect === "function") {
          onSelect(handle, { input: input });
        } else {
          input.value = "@" + handle;
        }
        hide();
        return;
      }
      var tok = atTokenAtCaretInput(input);
      if (!tok) return;
      var val = String(input.value || "");
      var before = val.slice(0, tok.atIndex);
      var after = val.slice(tok.endPos);
      var insert = "@" + handle;
      if (mode === "audience") {
        insert = "@" + handle;
        var needsComma =
          after.length === 0 || (after.charAt(0) !== "," && after.charAt(0) !== " ");
        input.value = before + insert + (needsComma ? ", " : "") + after.replace(/^[, ]*/, "");
      } else {
        input.value = before + insert + " " + after;
      }
      var pos = (before + insert + (mode === "audience" ? ", " : " ")).length;
      if (input.setSelectionRange) input.setSelectionRange(pos, pos);
      input.focus();
      hide();
      if (typeof onSelect === "function") onSelect(handle, { input: input });
    }

    function selectIndex(idx) {
      var opts = listEl.querySelectorAll(".handle-typeahead-option");
      if (!opts.length || idx < 0 || idx >= opts.length) return;
      var h =
        opts[idx].getAttribute("data-handle") ||
        opts[idx].getAttribute("data-profile-handle") ||
        "";
      insertAtToken(h);
    }

    function shouldTriggerFullSearch(raw) {
      var q = String(raw || "").trim().slice(0, 32);
      if (!q) return false;
      if (q.charAt(0) === "@") {
        return prefixSearchable(q.slice(1)) !== "";
      }
      return q.length >= MIN_PREFIX;
    }

    function runFetch() {
      if (trigger === "full") {
        var q = String(input.value || "").trim().slice(0, 32);
        if (!shouldTriggerFullSearch(q)) {
          hide();
          return;
        }
        var token = ++fetchToken;
        fetchGlobal(q)
          .then(function (items) {
            if (token !== fetchToken) return;
            renderOptions(listEl, items, defaultGlobalOption);
          })
          .catch(function () {
            hide();
          });
        return;
      }

      var tok = atTokenAtCaretInput(input);
      if (!tok) {
        hide();
        return;
      }
      var prefix = prefixSearchable(tok.prefix);
      if (!prefix) {
        hide();
        return;
      }
      var token = ++fetchToken;
      var p =
        mode === "audience"
          ? fetchAudience(prefix)
          : mode === "mention"
            ? fetchMention(getContext(), prefix)
            : fetchGlobal("@" + prefix);
      p.then(function (items) {
        if (token !== fetchToken) return;
        renderOptions(listEl, items, options.renderOption || defaultMentionOption);
      }).catch(function () {
        hide();
      });
    }

    var debouncedFetch = debounce(runFetch, options.debounceMs || DEBOUNCE_MS);

    function onInput() {
      debouncedFetch();
    }

    function onKeydown(e) {
      if (listEl.hidden) return;
      var opts = listEl.querySelectorAll(".handle-typeahead-option");
      if (!opts.length) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        activeIndex = Math.min(opts.length - 1, activeIndex + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        activeIndex = Math.max(0, activeIndex - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        if (activeIndex >= 0) {
          selectIndex(activeIndex);
        } else if (opts.length) {
          selectIndex(0);
        }
        return;
      } else if (e.key === "Escape") {
        e.stopPropagation();
        hide();
        return;
      } else {
        return;
      }
      opts.forEach(function (el, i) {
        el.classList.toggle("is-active", i === activeIndex);
      });
    }

    listEl.addEventListener("mousedown", function (e) {
      e.preventDefault();
    });

    listEl.addEventListener("click", function (e) {
      var btn = e.target.closest(".handle-typeahead-option");
      if (!btn) return;
      e.preventDefault();
      insertAtToken(
        btn.getAttribute("data-handle") || btn.getAttribute("data-profile-handle") || ""
      );
    });

    input.addEventListener("input", onInput);
    input.addEventListener("keydown", onKeydown);
    input.addEventListener("blur", function () {
      global.setTimeout(hide, 150);
    });

    return { destroy: hide, refresh: runFetch, hide: hide, listEl: listEl };
  }

  function bindMentionOverlayTypeahead(options) {
    options = options || {};
    var field = options.field;
    var fieldType = options.fieldType === "input" ? "input" : "editor";
    if (!field) return { destroy: function () {} };

    var listEl =
      options.listEl ||
      createOverlayListbox(
        options.listClass ||
          "handle-typeahead-overlay handle-typeahead-list create-post-mention-suggestions",
        options.ariaLabel || "Mention suggestions"
      );
    var getContext = options.getContext || function () { return {}; };
    var fallbackGlobal = options.fallbackGlobal === true;
    var collapsedLimit = Math.max(1, Number(options.overlayLimit) || 3);
    var showAllTemplate = options.showAllLabel || "Show all ({count})";
    var renderMentionItem = options.renderOption || defaultMentionOption;
    var renderGlobalItem = options.renderGlobalOption || defaultMentionOption;
    var currentRenderItem = renderMentionItem;
    var fetchToken = 0;
    var activeIndex = -1;
    var allItems = [];
    var expanded = false;

    function currentCaretRect() {
      return fieldType === "input" ? caretClientRectForInput(field) : caretClientRect(field);
    }

    function currentAtToken() {
      return fieldType === "input" ? atTokenAtCaretInput(field) : atTokenAtCaretEditor(field);
    }

    function syncOverlayPosition() {
      if (listEl.hidden) return;
      positionOverlayList(listEl, field, currentCaretRect());
    }

    function hide() {
      activeIndex = -1;
      allItems = [];
      expanded = false;
      listEl.innerHTML = "";
      listEl.hidden = true;
      listEl.style.left = "";
      listEl.style.top = "";
    }

    function paintOptions() {
      if (!allItems.length) {
        hide();
        return;
      }
      var visible = expanded ? allItems : allItems.slice(0, collapsedLimit);
      var html = visible.map(currentRenderItem).join("");
      if (!expanded && allItems.length > collapsedLimit) {
        html +=
          '<li role="option"><button type="button" class="handle-typeahead-option handle-typeahead-show-all" data-action="show-all">' +
          escapeHtml(formatShowAllLabel(showAllTemplate, allItems.length)) +
          "</button></li>";
      }
      listEl.innerHTML = html;
      listEl.hidden = false;
      listEl.classList.toggle("is-expanded", expanded);
      syncOverlayPosition();
      global.requestAnimationFrame(syncOverlayPosition);
      activeIndex = -1;
    }

    function showItems(items, useGlobalRenderer) {
      allItems = Array.isArray(items) ? items : [];
      expanded = false;
      currentRenderItem = useGlobalRenderer ? renderGlobalItem : renderMentionItem;
      paintOptions();
    }

    function expandAll() {
      expanded = true;
      paintOptions();
    }

    function insertMention(handle) {
      handle = stripAt(handle);
      if (!handle) return;
      if (fieldType === "input") {
        var tokIn = atTokenAtCaretInput(field);
        if (!tokIn) return;
        var val = String(field.value || "");
        var before = val.slice(0, tokIn.atIndex);
        var after = val.slice(tokIn.endPos);
        field.value = before + "@" + handle + " " + after;
        var pos = (before + "@" + handle + " ").length;
        if (field.setSelectionRange) field.setSelectionRange(pos, pos);
        field.focus();
        hide();
        return;
      }
      var sel = global.getSelection();
      if (!sel || !sel.rangeCount) return;
      var range = sel.getRangeAt(0);
      if (!field.contains(range.commonAncestorContainer)) return;
      var tokEd = atTokenAtCaretEditor(field);
      if (!tokEd) return;
      var startPos = walkCharOffsetToNode(field, tokEd.atIndex);
      if (!startPos) return;
      var del = document.createRange();
      del.setStart(startPos.node, startPos.offset);
      del.setEnd(range.endContainer, range.endOffset);
      del.deleteContents();
      var tn = document.createTextNode("@" + handle + " ");
      del.insertNode(tn);
      sel.removeAllRanges();
      var nr = document.createRange();
      nr.setStartAfter(tn);
      nr.collapse(true);
      sel.addRange(nr);
      hide();
    }

    function fetchGlobalFallback(prefix, token) {
      return fetchGlobal("@" + prefix).then(function (globalItems) {
        if (token !== fetchToken) return;
        showItems(globalItems, true);
      });
    }

    function runFetch() {
      var tok = currentAtToken();
      if (!tok) {
        hide();
        return;
      }
      var prefix = prefixSearchable(tok.prefix);
      if (!prefix) {
        hide();
        return;
      }
      var ctx = getContext();
      var token = ++fetchToken;
      if (!ctx.entityId) {
        if (!fallbackGlobal) {
          hide();
          return;
        }
        fetchGlobalFallback(prefix, token).catch(function () {
          hide();
        });
        return;
      }
      fetchMention(ctx, prefix)
        .then(function (items) {
          if (token !== fetchToken) return;
          if ((!items || !items.length) && fallbackGlobal) {
            return fetchGlobalFallback(prefix, token);
          }
          showItems(items, false);
        })
        .catch(function () {
          if (!fallbackGlobal) {
            hide();
            return;
          }
          fetchGlobalFallback(prefix, token).catch(function () {
            hide();
          });
        });
    }

    function listOptions() {
      return listEl.querySelectorAll(".handle-typeahead-option");
    }

    function activateOption(btn) {
      if (!btn) return;
      if (btn.getAttribute("data-action") === "show-all") {
        expandAll();
        return;
      }
      insertMention(btn.getAttribute("data-handle") || "");
    }

    function selectIndex(idx) {
      var opts = listOptions();
      if (!opts.length || idx < 0 || idx >= opts.length) return;
      activateOption(opts[idx]);
    }

    var debouncedFetch = debounce(runFetch, options.debounceMs || DEBOUNCE_MS);

    function onKeydown(e) {
      if (listEl.hidden) {
        if (e.key === "Escape") hide();
        return;
      }
      var opts = listOptions();
      if (!opts.length) {
        if (e.key === "Escape") hide();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        activeIndex = Math.min(opts.length - 1, activeIndex + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        activeIndex = Math.max(0, activeIndex - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        if (activeIndex >= 0) {
          selectIndex(activeIndex);
        } else if (opts.length) {
          selectIndex(0);
        }
        return;
      } else if (e.key === "Escape") {
        e.stopPropagation();
        hide();
        return;
      } else {
        return;
      }
      opts.forEach(function (el, i) {
        el.classList.toggle("is-active", i === activeIndex);
      });
    }

    function onReposition() {
      syncOverlayPosition();
    }

    field.addEventListener("input", debouncedFetch);
    field.addEventListener("keyup", debouncedFetch);
    field.addEventListener("keydown", onKeydown);
    field.addEventListener("blur", function () {
      global.setTimeout(hide, 150);
    });

    listEl.addEventListener("mousedown", function (e) {
      e.preventDefault();
    });

    listEl.addEventListener("click", function (e) {
      var btn = e.target.closest(".handle-typeahead-option");
      if (!btn) return;
      e.preventDefault();
      activateOption(btn);
    });

    global.addEventListener("resize", onReposition, { passive: true });
    global.addEventListener("scroll", onReposition, true);

    function destroy() {
      global.removeEventListener("resize", onReposition);
      global.removeEventListener("scroll", onReposition, true);
      if (listEl.parentNode) listEl.parentNode.removeChild(listEl);
      hide();
    }

    return { destroy: destroy, hide: hide, refresh: runFetch, listEl: listEl };
  }

  function bindEditorTypeahead(options) {
    options = options || {};
    if (!options.editor || options.overlay === false) return { destroy: function () {} };
    return bindMentionOverlayTypeahead(
      Object.assign({}, options, {
        field: options.editor,
        fieldType: "editor",
        listClass:
          options.listClass ||
          "handle-typeahead-overlay handle-typeahead-list create-post-mention-suggestions",
        fallbackGlobal: options.fallbackGlobal === true,
      })
    );
  }

  global.AppHandleTypeahead = {
    MIN_PREFIX: MIN_PREFIX,
    prefixSearchable: prefixSearchable,
    stripAt: stripAt,
    bindInput: bindInputTypeahead,
    bindEditor: bindEditorTypeahead,
    fetchGlobal: fetchGlobal,
    fetchAudience: fetchAudience,
    fetchMention: fetchMention,
  };
})(window);

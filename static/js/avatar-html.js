/**
 * Shared avatar markup — mirrors `snippets::avatar_not_found_html` + rail entity rings.
 */
(function (global) {
  "use strict";

  var NOT_FOUND_INNER =
    '<span class="app-avatar-not-found" aria-hidden="true">' +
    '<svg viewBox="0 0 40 40" width="20" height="20" xmlns="http://www.w3.org/2000/svg">' +
    '<circle cx="20" cy="20" r="19" fill="rgba(3,4,8,0.94)"/>' +
    '<path d="M20 28.8a1.35 1.35 0 1 1 0 2.7 1.35 1.35 0 0 1 0-2.7zm0-20.4c-3.55 0-6.1 2.05-6.1 5.25a1.25 1.25 0 0 0 2.5 0c0-1.68 1.36-2.75 3.6-2.75 2.31 0 3.72 1.02 3.72 2.62 0 1.11-.7 1.93-2.31 3.11-2.19 1.61-3.16 2.95-3.16 5.26v1.13a1.25 1.25 0 1 0 2.5 0v-1.13c0-1.36.48-2.07 2.14-3.29 2.05-1.5 3.33-2.82 3.33-5.08 0-3.15-2.66-5.12-6.22-5.12z" fill="var(--text)"/>' +
    "</svg></span>";

  function escapeHtml(s) {
    return String(s || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function avatarSrcFromStored(stored) {
    var t = String(stored || "").trim();
    if (!t) return "";
    if (/^https?:\/\//i.test(t)) return t;
    if (t.charAt(0) === "/") return t;
    return "/" + t.replace(/^\/+/, "");
  }

  function railSlotFrameClass(typeKey) {
    var t = String(typeKey || "").trim();
    if (t === "human") return " app-rail-slot-frame--human";
    if (t === "digital") return " app-rail-slot-frame--digital";
    if (t === "community") return " app-rail-slot-frame--community";
    if (t === "non_profit") return " app-rail-slot-frame--non_profit";
    if (t === "profit") return " app-rail-slot-frame--profit";
    if (t === "gathering") return " app-rail-slot-frame--gathering";
    return "";
  }

  function searchResultAvatarHtml(avatarPath, typeKey) {
    var frameClass =
      "brand-frame brand-frame--toolbar app-rail-slot-frame search-add-result-avatar-frame" +
      railSlotFrameClass(typeKey);
    var src = avatarSrcFromStored(avatarPath);
    if (src) {
      return (
        '<span class="' +
        frameClass +
        '" aria-hidden="true">' +
        '<img class="search-add-result-avatar app-rail-slot-img" src="' +
        escapeHtml(src) +
        '" alt="" width="24" height="24" loading="lazy" decoding="async" referrerpolicy="no-referrer" />' +
        "</span>"
      );
    }
    return (
      '<span class="' +
      frameClass +
      '" aria-hidden="true">' +
      NOT_FOUND_INNER +
      "</span>"
    );
  }

  global.AppAvatarHtml = {
    notFoundInner: NOT_FOUND_INNER,
    railSlotFrameClass: railSlotFrameClass,
    avatarSrcFromStored: avatarSrcFromStored,
    searchResultAvatarHtml: searchResultAvatarHtml,
    /** Replace a broken portrait `<img>` with the canonical ? placeholder. */
    imgOnError: function (img) {
      if (!img || img.getAttribute("data-avatar-fallback") === "1") return;
      img.setAttribute("data-avatar-fallback", "1");
      var wrap = document.createElement("span");
      wrap.innerHTML = NOT_FOUND_INNER;
      var node = wrap.firstElementChild;
      if (node) img.replaceWith(node);
    },
  };
  global.__avatarImgFallback = global.AppAvatarHtml.imgOnError;
})(window);

/**
 * Shared OG preview card helpers — domain-band withholding, click-to-reveal, http(s) image gate.
 */
(function (global) {
  "use strict";

  function isSafeHttpImageUrl(url) {
    var u = String(url || "").trim();
    if (!u) return false;
    var lower = u.toLowerCase();
    return lower.indexOf("http://") === 0 || lower.indexOf("https://") === 0;
  }

  function bindRevealClick(root, esc) {
    (root || document).addEventListener("click", function (ev) {
      var btn = ev.target.closest(".public-post-og-reveal-btn");
      if (!btn) return;
      ev.preventDefault();
      ev.stopPropagation();
      var url =
        btn.getAttribute("data-og-image-url") ||
        (btn.closest("[data-og-image-url]")
          ? btn.closest("[data-og-image-url]").getAttribute("data-og-image-url")
          : "");
      url = String(url || "").trim();
      if (!isSafeHttpImageUrl(url)) return;
      var og = btn.closest(".public-post-og");
      if (!og) return;
      var media = og.querySelector(".public-post-og-media");
      if (!media) return;
      media.innerHTML =
        '<img class="public-post-og-image" src="' +
        esc(url) +
        '" alt="" loading="lazy" decoding="async" />';
      og.classList.remove("public-post-og--image-withheld");
      og.classList.remove("public-post-og--no-image");
    });
  }

  function mediaInnerFromPreview(data, esc, defaults) {
    var d = defaults || {};
    var img = data && data.og_image_url ? String(data.og_image_url).trim() : "";
    var revealUrl =
      data && data.og_image_reveal_url ? String(data.og_image_reveal_url).trim() : "";
    var allowed = !data || data.preview_image_allowed !== false;
    var label =
      (data && data.og_preview_reveal_label) ||
      d.revealLabel ||
      "Show preview";
    var hint =
      (data && data.og_preview_withheld_hint) || d.withheldHint || "";

    if (img && isSafeHttpImageUrl(img) && allowed) {
      return {
        inner:
          '<img class="public-post-og-image" src="' +
          esc(img) +
          '" alt="" loading="lazy" decoding="async" />',
        withheld: false,
        hasImage: true,
      };
    }
    if (revealUrl && isSafeHttpImageUrl(revealUrl)) {
      var hintHtml = hint
        ? '<p class="public-post-og-withheld-hint">' + esc(hint) + "</p>"
        : "";
      return {
        inner:
          '<button type="button" class="public-post-og-reveal-btn" data-og-image-url="' +
          esc(revealUrl) +
          '">' +
          esc(label) +
          "</button>" +
          hintHtml,
        withheld: true,
        hasImage: true,
      };
    }
    return { inner: "", withheld: false, hasImage: false };
  }

  function ogBlockClass(media) {
    if (media.withheld) return "public-post-og public-post-og--image-withheld";
    if (media.hasImage) return "public-post-og";
    return "public-post-og public-post-og--no-image";
  }

  function buildOgInnerHtml(title, site, media, esc) {
    var mediaWrap =
      media.hasImage || media.withheld
        ? '<div class="public-post-og-media">' + media.inner + "</div>"
        : '<div class="public-post-og-media" aria-hidden="true"></div>';
    return (
      '<div class="' +
      ogBlockClass(media) +
      '">' +
      mediaWrap +
      '<div class="public-post-og-copy"><div class="public-post-og-title">' +
      esc(title) +
      '</div><div class="public-post-og-site">' +
      esc(site) +
      "</div></div></div>"
    );
  }

  function applyOgPreviewToRoot(ogRoot, data, esc, defaults) {
    if (!ogRoot) return;
    var media = mediaInnerFromPreview(data, esc, defaults);
    var title = (data && (data.og_title || data.url_canonical)) || "";
    var site = (data && data.og_site_name) || "";
    var mediaEl = ogRoot.querySelector(".public-post-og-media");
    if (mediaEl) {
      if (media.hasImage || media.withheld) {
        mediaEl.innerHTML = media.inner;
        mediaEl.removeAttribute("aria-hidden");
      }
    }
    ogRoot.className = ogBlockClass(media);
    var titleEl = ogRoot.querySelector(".public-post-og-title");
    if (titleEl && title) titleEl.textContent = title;
    var siteEl = ogRoot.querySelector(".public-post-og-site");
    if (siteEl && site) siteEl.textContent = site;
  }

  global.OgPreviewClient = {
    isSafeHttpImageUrl: isSafeHttpImageUrl,
    bindRevealClick: bindRevealClick,
    mediaInnerFromPreview: mediaInnerFromPreview,
    buildOgInnerHtml: buildOgInnerHtml,
    applyOgPreviewToRoot: applyOgPreviewToRoot,
  };
})(typeof window !== "undefined" ? window : globalThis);

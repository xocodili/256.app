(function () {
  "use strict";

  var Og = window.OgPreviewClient || {};

  function esc(s) {
    return String(s || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function firstUrlFromCard(card) {
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

  function publicOgLoadingHtml() {
    return window.brandCardLoader && window.brandCardLoader.publicOgLoadingHtml
      ? window.brandCardLoader.publicOgLoadingHtml()
      : "";
  }

  function publicPostCardOgAnchor(card) {
    var head = card.querySelector(".public-post-head");
    if (head) return head;
    return card.querySelector(".public-post-body");
  }

  function insertPublicOgFragment(card, html) {
    var anchor = publicPostCardOgAnchor(card);
    if (!anchor) return;
    anchor.insertAdjacentHTML("afterend", html);
  }

  function run() {
    if (Og.bindRevealClick) {
      Og.bindRevealClick(document, esc);
    }
    var cards = document.querySelectorAll(".public-post-card");
    if (!cards.length) return;
    cards.forEach(function (card) {
      var url = firstUrlFromCard(card);
      if (!url) return;
      if (!card.getAttribute("data-link-url")) {
        card.setAttribute("data-link-url", url);
      }
      if (card.querySelector(".public-post-og-image")) return;
      if (card.querySelector(".public-post-og--image-withheld")) return;
      if (card.querySelector(".public-post-og-loading")) return;
      if (card.getAttribute("data-og-fetch-pending") === "1") return;

      var body = card.querySelector(".public-post-body");
      if (!body) return;

      var existingOg = card.querySelector(".public-post-og");
      if (!existingOg) {
        insertPublicOgFragment(card, publicOgLoadingHtml());
      }
      card.setAttribute("data-og-fetch-pending", "1");

      function clearOgLoading() {
        var el = card.querySelector(".public-post-og-loading");
        if (el) el.remove();
      }

      fetch("/api/public/content-links/preview-og?url=" + encodeURIComponent(url), {
        method: "GET",
        credentials: "same-origin",
        headers: { Accept: "application/json" },
      })
        .then(function (r) {
          if (!r.ok) throw new Error("preview_failed");
          return r.json();
        })
        .then(function (data) {
          card.removeAttribute("data-og-fetch-pending");
          if (!data || data.domain_blocked) {
            clearOgLoading();
            return;
          }
          var canonical = (data.url_canonical || url || "").trim();
          if (!canonical) {
            clearOgLoading();
            return;
          }
          var title = data.og_title || canonical;
          var site = data.og_site_name || "";
          var defaults = {
            revealLabel: data.og_preview_reveal_label || "Show preview",
            withheldHint: data.og_preview_withheld_hint || "",
          };
          var media =
            Og.mediaInnerFromPreview && Og.mediaInnerFromPreview(data, esc, defaults);
          if (!media) {
            media = { inner: "", withheld: false, hasImage: false };
          }
          var ogRoot = card.querySelector(".public-post-og");
          if (ogRoot) {
            if (Og.applyOgPreviewToRoot) {
              Og.applyOgPreviewToRoot(ogRoot, data, esc, defaults);
            }
            clearOgLoading();
            return;
          }
          var ogInner =
            Og.buildOgInnerHtml && Og.buildOgInnerHtml(title, site, media, esc);
          if (!ogInner) {
            ogInner =
              '<div class="public-post-og public-post-og--no-image">' +
              '<div class="public-post-og-media" aria-hidden="true"></div>' +
              '<div class="public-post-og-copy"><div class="public-post-og-title">' +
              esc(title) +
              '</div><div class="public-post-og-site">' +
              esc(site) +
              "</div></div></div>";
          }
          var html =
            '<a class="public-post-og-link" href="' +
            esc(canonical) +
            '" target="_blank" rel="noopener noreferrer" aria-label="' +
            esc(title || "Open link") +
            '">' +
            ogInner +
            "</a>";
          var loadEl = card.querySelector(".public-post-og-loading");
          if (loadEl) {
            loadEl.insertAdjacentHTML("beforebegin", html);
            loadEl.remove();
          } else {
            insertPublicOgFragment(card, html);
          }
        })
        .catch(function () {
          card.removeAttribute("data-og-fetch-pending");
          clearOgLoading();
        });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", run);
  } else {
    run();
  }
})();

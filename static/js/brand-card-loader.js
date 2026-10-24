/**
 * Card / inline loaders: spinning site smiley inside a thin spectrum frame (matches `snippets::card_loader`).
 */
(function (global) {
  "use strict";

  var FACE =
    '<circle class="smiley-disc brand-loader-face" cx="128" cy="128" r="108"/>' +
    '<circle class="brand-loader-eye brand-loader-eye-left eye eye-left" cx="90" cy="104" r="15"/>' +
    '<circle class="brand-loader-eye brand-loader-eye-right eye eye-right" cx="166" cy="104" r="15"/>' +
    '<path class="brand-loader-mouth mouth" d="M 76 160 Q 128 198 180 160"/>';

  function loaderSvgInner() {
    return '<g class="brand-loader-spin">' + FACE + "</g>";
  }

  function cardLoaderHtml() {
    return (
      '<div class="card-loader"><span class="brand-frame brand-frame--loader">' +
      '<svg class="brand-loader-svg" viewBox="0 0 256 256" role="img" aria-hidden="true" xmlns="http://www.w3.org/2000/svg">' +
      loaderSvgInner() +
      "</svg></span></div>"
    );
  }

  function feedControlsLoaderHtml() {
    return (
      '<span class="brand-frame brand-frame--loader">' +
      '<svg class="brand-loader-svg" viewBox="0 0 256 256" role="img" aria-hidden="true" xmlns="http://www.w3.org/2000/svg">' +
      loaderSvgInner() +
      "</svg></span>"
    );
  }

  function publicOgLoadingHtml() {
    return (
      '<div class="public-post-og-loading" role="status" aria-live="polite" aria-busy="true" aria-label="Loading link preview">' +
      cardLoaderHtml() +
      "</div>"
    );
  }

  function panelCardLoaderHtml() {
    return (
      '<section class="card app-card">' +
      '<div class="card-accent" role="presentation"></div>' +
      cardLoaderHtml() +
      "</section>"
    );
  }

  global.brandCardLoader = {
    cardLoaderHtml: cardLoaderHtml,
    feedControlsLoaderHtml: feedControlsLoaderHtml,
    publicOgLoadingHtml: publicOgLoadingHtml,
    panelCardLoaderHtml: panelCardLoaderHtml,
  };
})(typeof window !== "undefined" ? window : globalThis);

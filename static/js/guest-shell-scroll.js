/**
 * Guest shells scroll inside #guest-split-auth / #guest-split-public (viewport-lock body).
 * Reset scroll on navigation so council profile links always open at the top.
 */
(function () {
  "use strict";

  if ("scrollRestoration" in history) {
    history.scrollRestoration = "manual";
  }

  function guestScrollRoot() {
    return (
      document.getElementById("guest-split-public") ||
      document.getElementById("guest-split-auth")
    );
  }

  function resetGuestShellScroll() {
    var root = guestScrollRoot();
    if (root) {
      root.scrollTop = 0;
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", resetGuestShellScroll);
  } else {
    resetGuestShellScroll();
  }

  window.addEventListener("pageshow", function (event) {
    if (event.persisted) {
      resetGuestShellScroll();
    }
  });
})();

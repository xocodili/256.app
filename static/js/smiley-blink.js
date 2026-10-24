/**
 * Brand smiley: bilateral blinks at roughly human rate (~12–18/min) and an occasional wink (~30s).
 * Skipped when prefers-reduced-motion. Requires .eye-left / .eye-right on the SVG circles.
 */
(function () {
  "use strict";

  if (
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches
  ) {
    return;
  }

  var svgs = document.querySelectorAll(".brand-hero-svg");
  if (!svgs.length) return;

  var pairs = [];
  for (var i = 0; i < svgs.length; i++) {
    var svg = svgs[i];
    var left = svg.querySelector(".eye-left");
    var right = svg.querySelector(".eye-right");
    if (left && right) pairs.push([left, right]);
  }
  if (!pairs.length) return;

  var busy = false;

  function releaseBusy(ms) {
    setTimeout(function () {
      busy = false;
    }, ms);
  }

  /** Typical human IBI ~2–6s (roughly 10–20 blinks/min). */
  function nextBlinkDelayMs() {
    return 2000 + Math.random() * 4000;
  }

  /** Wink every ~25–38s (centred near 30s). */
  function nextWinkDelayMs() {
    return 25000 + Math.random() * 13000;
  }

  function blinkBoth() {
    if (busy) return;
    busy = true;
    var hold = 95 + Math.random() * 75;
    for (var j = 0; j < pairs.length; j++) {
      pairs[j][0].classList.add("is-closed");
      pairs[j][1].classList.add("is-closed");
    }
    setTimeout(function () {
      for (var k = 0; k < pairs.length; k++) {
        pairs[k][0].classList.remove("is-closed");
        pairs[k][1].classList.remove("is-closed");
      }
      releaseBusy(100);
    }, hold);
  }

  function winkLeft() {
    if (busy) return;
    busy = true;
    var hold = 140 + Math.random() * 100;
    var left0 = pairs[0][0];
    left0.classList.add("is-closed");
    setTimeout(function () {
      left0.classList.remove("is-closed");
      releaseBusy(100);
    }, hold);
  }

  function scheduleBlinkLoop() {
    setTimeout(function () {
      blinkBoth();
      scheduleBlinkLoop();
    }, nextBlinkDelayMs());
  }

  function scheduleWinkLoop() {
    setTimeout(function () {
      winkLeft();
      scheduleWinkLoop();
    }, nextWinkDelayMs());
  }

  /* First blink after a short settle; first wink not immediately. */
  setTimeout(function () {
    scheduleBlinkLoop();
  }, 800 + Math.random() * 1400);

  setTimeout(function () {
    scheduleWinkLoop();
  }, 12000 + Math.random() * 10000);
})();

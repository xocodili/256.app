/**
 * Public entity profile: loads JSON from boot.card_url.
 * Expects #entity-public-ui (not_found, network_error, boot_invalid) and #entity-public-boot ({ card_url }).
 */
(function () {
  "use strict";

  var REASON_NOT_FOUND = "not_found";

  function text(el, s) {
    el.textContent = s;
  }

  function show(el, on) {
    el.hidden = !on;
    if (on) el.removeAttribute("hidden");
    else el.setAttribute("hidden", "");
  }

  function parseJsonScript(id) {
    var el = document.getElementById(id);
    if (!el) return { ok: false, data: null };
    try {
      return { ok: true, data: JSON.parse(el.textContent) };
    } catch (_) {
      return { ok: false, data: null };
    }
  }

  function showError(loadingEl, root, errEl, message) {
    show(loadingEl, false);
    show(root, false);
    text(errEl, message || "");
    show(errEl, true);
  }

  function avatarImgSrc(stored) {
    var s = String(stored || "").trim();
    if (!s) return "";
    if (/^https?:\/\//i.test(s)) return s;
    return s.charAt(0) === "/" ? s : "/" + s;
  }

  function roundedHalfSteps(raw) {
    var n = Number(raw);
    if (!Number.isFinite(n)) return 0;
    if (n < 0) n = 0;
    if (n > 5) n = 5;
    return Math.round(n * 2);
  }

  function bindConstitutionExpander(root, ui) {
    if (!root || root.getAttribute("data-constitution-bound") === "1") return;
    var body = root.querySelector("#profile-constitution-body");
    var btn = root.querySelector("[data-profile-constitution-expand]");
    if (!body || !btn) return;
    root.setAttribute("data-constitution-bound", "1");
    body.classList.add("is-clamped");
    var showMore = (ui && ui.constitution_show_more) || "Show more …";
    var showLess = (ui && ui.constitution_show_less) || "Show less";
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
      if (!body.classList.contains("is-expanded")) body.classList.add("is-clamped");
    }
    btn.addEventListener("click", function () {
      var expanded = body.classList.toggle("is-expanded");
      body.classList.toggle("is-clamped", !expanded);
      btn.setAttribute("aria-expanded", expanded ? "true" : "false");
      btn.textContent = expanded ? showLess : showMore;
    });
    syncButton();
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(function () {
        requestAnimationFrame(syncButton);
      });
    }
  }

  function appendConstitutionSection(root, data, ui) {
    var sections = data.constitution_sections;
    if (!sections || !sections.length) {
      if (!data.constitution_empty) return;
      var emptySec = document.createElement("section");
      emptySec.className = "profile-block profile-constitution-block";
      var emptyH = document.createElement("h2");
      text(emptyH, data.constitution_heading || "Our Constitution");
      emptySec.appendChild(emptyH);
      var emptyP = document.createElement("p");
      emptyP.className = "profile-value profile-brand-text profile-constitution-empty";
      text(emptyP, data.constitution_empty);
      emptySec.appendChild(emptyP);
      root.appendChild(emptySec);
      return;
    }
    var showTierLabels = sections.length > 1;
    var body = document.createElement("div");
    body.className = "profile-constitution-body";
    body.id = "profile-constitution-body";
    sections.forEach(function (section, idx) {
      if (showTierLabels && idx > 0) {
        var tierH = document.createElement("h3");
        tierH.className = "profile-constitution-tier";
        var tierPrefix = document.createElement("span");
        text(tierPrefix, (data.constitution_tier_from || "Inherited from") + " ");
        tierH.appendChild(tierPrefix);
        var tierHandle = document.createElement("span");
        tierHandle.className = "profile-constitution-tier-handle";
        text(tierHandle, "@" + String(section.community_handle || ""));
        tierH.appendChild(tierHandle);
        body.appendChild(tierH);
      }
      var list = document.createElement("ol");
      list.className = "profile-constitution-list";
      (section.clauses || []).forEach(function (clause) {
        var li = document.createElement("li");
        var point = document.createElement("span");
        point.className = "profile-constitution-point mono";
        text(point, "#" + String(clause.point));
        li.appendChild(point);
        li.appendChild(document.createTextNode(" "));
        var item = document.createElement("span");
        item.className = "profile-constitution-item";
        text(item, String(clause.item || "").trim());
        li.appendChild(item);
        list.appendChild(li);
      });
      body.appendChild(list);
    });
    var sec = document.createElement("section");
    sec.className = "profile-block profile-constitution-block";
    var h2 = document.createElement("h2");
    text(h2, data.constitution_heading || "Our Constitution");
    sec.appendChild(h2);
    sec.appendChild(body);
    var expandBtn = document.createElement("button");
    expandBtn.type = "button";
    expandBtn.className = "profile-constitution-expand-btn";
    expandBtn.setAttribute("data-profile-constitution-expand", "1");
    expandBtn.setAttribute("aria-controls", "profile-constitution-body");
    expandBtn.setAttribute("aria-expanded", "false");
    expandBtn.hidden = true;
    text(expandBtn, (ui && ui.constitution_show_more) || "Show more …");
    sec.appendChild(expandBtn);
    root.appendChild(sec);
  }

  function buildCommunityPublicBody(root, data, ui) {
    if (data.community_members_summary_label) {
      var memSec = document.createElement("section");
      memSec.className = "profile-block";
      var memLine = document.createElement("h2");
      memLine.className = "profile-brand-base-size";
      text(memLine, data.community_members_summary_label);
      memSec.appendChild(memLine);
      root.appendChild(memSec);
    }
    if (data.about) {
      var aboutSec = document.createElement("section");
      aboutSec.className = "profile-block";
      var aboutP = document.createElement("p");
      aboutP.className = "profile-value profile-brand-text";
      text(aboutP, data.about);
      aboutSec.appendChild(aboutP);
      root.appendChild(aboutSec);
    }
    if (data.tagline) {
      var tagSec = document.createElement("section");
      tagSec.className = "profile-block";
      var tagH = document.createElement("h2");
      text(tagH, data.tagline_heading || "Our Intention");
      tagSec.appendChild(tagH);
      var tagP = document.createElement("p");
      tagP.className = "profile-value profile-brand-text";
      text(tagP, data.tagline);
      tagSec.appendChild(tagP);
      root.appendChild(tagSec);
    }
    appendConstitutionSection(root, data, ui);
  }

  function buildSsrStars(rating) {
    var steps = roundedHalfSteps(rating);
    var full = Math.floor(steps / 2);
    var hasHalf = steps % 2 === 1;
    var wrap = document.createElement("span");
    wrap.className = "ssr-stars ssr-stars--avatar";
    wrap.setAttribute("aria-label", "SSR " + (Number(rating || 0).toFixed(1)) + " out of 5");
    for (var i = 0; i < full; i += 1) {
      var star = document.createElement("span");
      star.className = "ssr-star ssr-star--full";
      star.setAttribute("aria-hidden", "true");
      star.textContent = "★";
      wrap.appendChild(star);
    }
    if (hasHalf) {
      var half = document.createElement("span");
      half.className = "ssr-star ssr-star--half";
      half.setAttribute("aria-hidden", "true");
      half.textContent = "☆";
      wrap.appendChild(half);
    }
    return wrap;
  }

  function buildCard(root, data, ui) {
    root.innerHTML = "";
    root.removeAttribute("data-constitution-bound");

    var head = document.createElement("div");
    head.className = "public-post-head";
    var identity = document.createElement("div");
    identity.className = "public-post-head-identity";

    var avatarLink = document.createElement("span");
    avatarLink.className = "public-post-avatar-link";
    var avatarFrame = document.createElement("span");
    avatarFrame.className = "brand-frame brand-frame--toolbar public-post-avatar";
    var avatarStack = document.createElement("span");
    avatarStack.className = "avatar-stack";
    if (data.avatar_path) {
      var img = document.createElement("img");
      img.className = "public-post-avatar-img";
      img.width = 64;
      img.height = 64;
      img.alt = data.handle ? "@" + data.handle : "";
      img.loading = "lazy";
      img.decoding = "async";
      img.referrerPolicy = "no-referrer";
      img.src = avatarImgSrc(data.avatar_path);
      avatarStack.appendChild(img);
    } else {
      var fallback = document.createElement("span");
      fallback.className = "app-avatar-not-found";
      fallback.innerHTML =
        '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="1.6"></circle><path d="M9.7 9.3a2.3 2.3 0 1 1 4.6 0c0 1.6-1.9 2.2-2.3 3.8" fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="1.6"></path><circle cx="12" cy="17.4" r="1" fill="currentColor"></circle></svg>';
      avatarStack.appendChild(fallback);
    }
    var typeKey = String(data.type_key || "");
    var brandStyleCard =
      typeKey === "profit" ||
      typeKey === "non_profit" ||
      typeKey === "gathering" ||
      typeKey === "community";
    var ratingNum =
      data.entity_rating != null && Number.isFinite(Number(data.entity_rating))
        ? Number(data.entity_rating)
        : 0;
    if (brandStyleCard) {
      avatarStack.appendChild(buildSsrStars(ratingNum));
    } else if (data.show_entity_rating && data.entity_rating != null) {
      avatarStack.appendChild(buildSsrStars(Number(data.entity_rating)));
    }
    avatarFrame.appendChild(avatarStack);
    avatarLink.appendChild(avatarFrame);
    identity.appendChild(avatarLink);

    var handleBlock = document.createElement("div");
    handleBlock.className = "public-post-handle-block";
    var handle = String(data.handle || "").trim();
    var handleHref = document.createElement("a");
    handleHref.className = "public-post-handle";
    if (handle) {
      handleHref.href = "/public/" + encodeURIComponent(handle);
    }
    var handleLine = document.createElement("span");
    handleLine.className = "public-post-handle-line";
    text(handleLine, "@" + handle);
    handleHref.appendChild(handleLine);
    handleBlock.appendChild(handleHref);

    var nameRow = document.createElement("span");
    nameRow.className = "public-post-name-row";
    if (data.display_name) {
      var displayName = document.createElement("span");
      displayName.className = "public-post-display-name";
      text(displayName, data.display_name);
      nameRow.appendChild(displayName);
    }
    var cc = String(data.country_code || "").trim();
    if (/^[A-Za-z]{2}$/.test(cc)) {
      var flag = document.createElement("span");
      flag.className = "public-post-flag-icon public-post-flag-inline fi fi-" + cc.toLowerCase();
      flag.setAttribute("data-country-code", cc.toUpperCase());
      flag.setAttribute("title", "Profile country " + cc.toUpperCase());
      flag.setAttribute("aria-hidden", "true");
      nameRow.appendChild(flag);
    }
    handleBlock.appendChild(nameRow);
    identity.appendChild(handleBlock);
    head.appendChild(identity);
    root.appendChild(head);

    if (typeKey === "community") {
      buildCommunityPublicBody(root, data, ui);
      return;
    }

    if (typeKey === "digital") {
      if (data.about) {
        var digitalAboutP = document.createElement("p");
        digitalAboutP.className = "profile-value entity-public-about entity-public-digital-about";
        text(digitalAboutP, data.about);
        root.appendChild(digitalAboutP);
      }
      return;
    }

    if (typeKey === "human" && data.human_activity_summary) {
      var activity = document.createElement("p");
      activity.className = "profile-human-activity-summary profile-value";
      text(activity, data.human_activity_summary);
      root.appendChild(activity);
    }

    if (data.trading_name) {
      var trade = document.createElement("p");
      trade.className = "entity-public-trading-name";
      text(trade, data.trading_name);
      root.appendChild(trade);
    }

    if (data.subtitle) {
      var sub = document.createElement("p");
      sub.className = "sub";
      text(sub, data.subtitle);
      root.appendChild(sub);
    }

    if (typeKey === "gathering" && data.gathering_rsvp_summary_label) {
      var rsvpSec = document.createElement("section");
      rsvpSec.className = "profile-block profile-gathering-rsvp-metrics";
      var rsvpLine = document.createElement("h2");
      rsvpLine.className = "profile-brand-base-size";
      text(rsvpLine, data.gathering_rsvp_summary_label);
      rsvpSec.appendChild(rsvpLine);
      root.appendChild(rsvpSec);
    }
    function appendAboutSection() {
      if (!data.about) return;
      var aboutSec = document.createElement("section");
      aboutSec.className = "profile-block";
      var aboutH = document.createElement("h2");
      text(aboutH, data.about_heading || "About");
      aboutSec.appendChild(aboutH);
      var aboutP = document.createElement("p");
      aboutP.className = "profile-value entity-public-about";
      text(aboutP, data.about);
      aboutSec.appendChild(aboutP);
      root.appendChild(aboutSec);
    }

    function appendTaglineSection() {
      if (!data.tagline) return;
      var tagSec = document.createElement("section");
      tagSec.className = "profile-block";
      var tagH = document.createElement("h2");
      text(tagH, data.tagline_heading || "Tagline");
      tagSec.appendChild(tagH);
      var tagP = document.createElement("p");
      tagP.className = "profile-value";
      text(tagP, data.tagline);
      tagSec.appendChild(tagP);
      root.appendChild(tagSec);
    }

    /* Brand-style profiles: About then intention; events use location instead of intention; communities match About + intention with members line up top. */
    if (brandStyleCard) {
      appendAboutSection();
      appendTaglineSection();
    } else {
      appendTaglineSection();
    }

    var typeSec = document.createElement("section");
    typeSec.className = "profile-block";
    var typeH = document.createElement("h2");
    text(typeH, data.type_heading);
    typeSec.appendChild(typeH);
    var typeP = document.createElement("p");
    typeP.className = "profile-value";
    text(typeP, data.type_label);
    typeSec.appendChild(typeP);
    root.appendChild(typeSec);

    if (!brandStyleCard) {
      appendAboutSection();
    }

    if (data.location_label && typeKey !== "gathering" && typeKey !== "human") {
      var locSec = document.createElement("section");
      locSec.className = "profile-block";
      var locH = document.createElement("h2");
      text(locH, data.location_heading);
      locSec.appendChild(locH);
      var locP = document.createElement("p");
      locP.className = "profile-value";
      text(locP, data.location_label);
      locSec.appendChild(locP);
      root.appendChild(locSec);
    }

    if (data.legal_name) {
      var legSec = document.createElement("section");
      legSec.className = "profile-block";
      var legH = document.createElement("h2");
      text(legH, data.legal_heading || "Registered name");
      legSec.appendChild(legH);
      var legP = document.createElement("p");
      legP.className = "profile-value";
      text(legP, data.legal_name);
      legSec.appendChild(legP);
      root.appendChild(legSec);
    }

    if (data.founded_display) {
      var fdSec = document.createElement("section");
      fdSec.className = "profile-block";
      var fdH = document.createElement("h2");
      text(fdH, data.founded_heading || "Founded");
      fdSec.appendChild(fdH);
      var fdP = document.createElement("p");
      fdP.className = "profile-value";
      text(fdP, data.founded_display);
      fdSec.appendChild(fdP);
      root.appendChild(fdSec);
    }

    if (data.website_url) {
      var webSec = document.createElement("section");
      webSec.className = "profile-block";
      var webH = document.createElement("h2");
      text(webH, data.website_heading || "Website");
      webSec.appendChild(webH);
      var webP = document.createElement("p");
      webP.className = "profile-value";
      var webA = document.createElement("a");
      webA.href = data.website_url;
      webA.target = "_blank";
      webA.rel = "noopener noreferrer nofollow";
      text(webA, data.website_url);
      webP.appendChild(webA);
      webSec.appendChild(webP);
      root.appendChild(webSec);
    }
  }

  function loadEntityPublicCard(options) {
    options = options || {};
    var loadingId = options.loadingId || "entity-card-loading";
    var rootId = options.rootId || "entity-card-root";
    var errId = options.errId || "entity-card-error";
    var uiScriptId = options.uiScriptId || "entity-public-ui";
    var bootScriptId = options.bootScriptId || "entity-public-boot";
    var shellId = options.shellId || "entity-public-card";
    var updateTitle = options.updateTitle !== false;

    var loadingEl = document.getElementById(loadingId);
    var root = document.getElementById(rootId);
    var errEl = document.getElementById(errId);
    if (!loadingEl || !root || !errEl) return;

    var uiParsed = parseJsonScript(uiScriptId);
    if (!uiParsed.ok) {
      showError(
        loadingEl,
        root,
        errEl,
        "This page could not load correctly. Refresh and try again."
      );
      return;
    }
    var ui = uiParsed.data || {};

    var bootParsed = parseJsonScript(bootScriptId);
    if (!bootParsed.ok) {
      showError(loadingEl, root, errEl, ui.boot_invalid);
      return;
    }

    var cardUrl = bootParsed.data && bootParsed.data.card_url;
    if (!cardUrl) {
      showError(loadingEl, root, errEl, ui.not_found);
      return;
    }

    fetch(cardUrl, {
      method: "GET",
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    })
      .then(function (res) {
        if (res.status === 404) {
          var err = new Error(REASON_NOT_FOUND);
          err.kind = REASON_NOT_FOUND;
          throw err;
        }
        if (!res.ok) {
          throw new Error("http_" + res.status);
        }
        return res.json();
      })
      .then(function (data) {
        if (updateTitle) {
          document.title = data.html_title || document.title;
        }
        var shell = document.getElementById(shellId);
        if (shell && data.type_key) {
          shell.className = shell.className
            .replace(/\bentity-profile-accent--\S+/g, "")
            .replace(/\s+/g, " ")
            .trim();
          shell.classList.add("entity-profile-accent--" + data.type_key);
        }
        buildCard(root, data, ui);
        show(loadingEl, false);
        show(root, true);
        bindConstitutionExpander(root, ui);
        show(errEl, false);
        var scrollRoot = document.getElementById("guest-split-public");
        if (scrollRoot) {
          scrollRoot.style.overflowAnchor = "none";
        }
      })
      .catch(function (e) {
        var msg =
          e && e.kind === REASON_NOT_FOUND ? ui.not_found : ui.network_error;
        showError(loadingEl, root, errEl, msg);
      });
  }

  function run() {
    loadEntityPublicCard({});
    loadEntityPublicCard({
      loadingId: "face-capture-artist-loading",
      rootId: "face-capture-artist-root",
      errId: "face-capture-artist-error",
      uiScriptId: "face-capture-artist-ui",
      bootScriptId: "face-capture-artist-boot",
      shellId: "face-capture-artist-card",
      updateTitle: false,
    });
  }

  window.AppEntityPublicCard = {
    load: loadEntityPublicCard,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", run);
  } else {
    run();
  }
})();

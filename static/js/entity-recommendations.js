/**
 * Entity recommendations for the Search/Add panel.
 * Fetches from `GET /me/entity-recommendations` (sectioned graph-walk sources).
 */
(function (global) {
  "use strict";

  var RECOMMENDATIONS_URL = "/me/entity-recommendations";
  var TYPE_ORDER = ["community", "gathering", "profit", "non_profit"];

  function escapeHtml(s) {
    return String(s || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function stripAt(handle) {
    return String(handle || "")
      .trim()
      .replace(/^@+/, "");
  }

  function sectionTitle(UI, sectionKey) {
    if (!UI) return sectionKey;
    if (sectionKey === "direct_connections") {
      return UI.search_recommendations_direct_title || "Based on your connections";
    }
    if (sectionKey === "users_like_you") {
      return UI.search_recommendations_inferred_title || "Based on users like you";
    }
    return sectionKey;
  }

  function searchResultAvatarHtml(avatarPath, typeKey) {
    if (global.AppAvatarHtml && global.AppAvatarHtml.searchResultAvatarHtml) {
      return global.AppAvatarHtml.searchResultAvatarHtml(avatarPath, typeKey);
    }
    return "";
  }

  function entityProfileAccentClass(typeKey) {
    var t = String(typeKey || "").trim();
    if (t === "human") return "entity-profile-accent--human";
    if (t === "digital") return "entity-profile-accent--digital";
    if (t === "community") return "entity-profile-accent--community";
    if (t === "non_profit") return "entity-profile-accent--non_profit";
    if (t === "profit") return "entity-profile-accent--profit";
    if (t === "gathering") return "entity-profile-accent--gathering";
    return "";
  }

  function renderSearchResultRow(it, opts) {
    opts = opts || {};
    var handle = stripAt(it && it.handle);
    var showHandle = "@" + handle;
    var label = String((it && it.label) || showHandle).trim();
    var typeKey = String((it && it.type_key) || "").trim();
    var accent = entityProfileAccentClass(typeKey);
    var itemClass = "search-add-result-item" + (accent ? " " + accent : "");
    var inner;
    if (opts.asButton) {
      inner =
        '<button type="button" class="search-add-result-link ' +
        escapeHtml(opts.buttonClass || "") +
        '" data-handle="' +
        escapeHtml(handle) +
        '">' +
        escapeHtml(showHandle) +
        "</button>";
    } else {
      inner =
        '<a class="feed-profile-link search-add-result-link" href="/home/profile/' +
        encodeURIComponent(handle) +
        '" data-profile-handle="' +
        escapeHtml(handle) +
        '">' +
        escapeHtml(showHandle) +
        "</a>";
    }
    return (
      '<li class="' +
      itemClass +
      '">' +
      searchResultAvatarHtml(it && it.avatar_path, typeKey) +
      inner +
      '<span class="search-add-result-sub">' +
      escapeHtml(label) +
      "</span>" +
      "</li>"
    );
  }

  function renderEntityList(container, rows) {
    if (!container) return;
    var items = Array.isArray(rows) ? rows : [];
    container.innerHTML = items
      .map(function (it) {
        return renderSearchResultRow(it);
      })
      .join("");
  }

  function flattenSectionItems(section) {
    var types = section && Array.isArray(section.types) ? section.types : [];
    var items = [];
    TYPE_ORDER.forEach(function (typeKey) {
      var bucket = types.find(function (t) {
        return String((t && t.type_key) || "").trim() === typeKey;
      });
      if (bucket && Array.isArray(bucket.items)) {
        bucket.items.forEach(function (it) {
          items.push(it);
        });
      }
    });
    return items;
  }

  function renderSection(UI, section) {
    var key = String((section && section.key) || "").trim();
    var items = flattenSectionItems(section);
    if (!items.length) return "";
    return (
      '<section class="search-add-recommendations-block" data-recommendation-section="' +
      escapeHtml(key) +
      '">' +
      '<h2 class="search-add-recommendations-title">' +
      escapeHtml(sectionTitle(UI, key)) +
      "</h2>" +
      '<ul class="search-add-results search-add-recommendations-list">' +
      items
        .map(function (it) {
          return renderSearchResultRow(it);
        })
        .join("") +
      "</ul>" +
      "</section>"
    );
  }

  function renderSections(rootEl, payload, UI) {
    if (!rootEl) return false;
    var sections = payload && Array.isArray(payload.sections) ? payload.sections : [];
    var html = sections
      .map(function (section) {
        return renderSection(UI, section);
      })
      .join("");
    if (!html) {
      rootEl.innerHTML = "";
      return false;
    }
    rootEl.innerHTML = html;
    return true;
  }

  function fetchRecommendations(perType) {
    var lim = Math.max(1, Math.min(6, Number(perType) || 6));
    return fetch(RECOMMENDATIONS_URL + "?per_type=" + encodeURIComponent(String(lim)), {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    }).then(function (r) {
      if (!r.ok) throw new Error("recommendations_failed");
      return r.json();
    });
  }

  function loadRecommendationsPanel(UI) {
    var sectionsEl = document.getElementById("search-add-recommendations-sections");
    var emptyEl = document.getElementById("search-add-recommendations-empty");
    if (!sectionsEl) return Promise.resolve();
    return fetchRecommendations(6)
      .then(function (payload) {
        var hasRows = renderSections(sectionsEl, payload, UI);
        if (!hasRows) {
          sectionsEl.innerHTML = "";
          if (emptyEl) {
            emptyEl.hidden = false;
            if (UI && UI.search_recommendations_empty) {
              emptyEl.textContent = UI.search_recommendations_empty;
            }
          }
          return;
        }
        if (emptyEl) emptyEl.hidden = true;
      })
      .catch(function () {
        sectionsEl.innerHTML = "";
        if (emptyEl) {
          emptyEl.hidden = false;
          emptyEl.textContent =
            (UI && UI.search_recommendations_load_failed) ||
            "Could not load recommendations. Check your connection and try again.";
        }
      });
  }

  global.AppEntityRecommendations = {
    RECOMMENDATIONS_URL: RECOMMENDATIONS_URL,
    renderEntityList: renderEntityList,
    renderSearchResultRow: renderSearchResultRow,
    entityProfileAccentClass: entityProfileAccentClass,
    searchResultAvatarHtml: searchResultAvatarHtml,
    fetchRecommendations: fetchRecommendations,
    loadRecommendationsPanel: loadRecommendationsPanel,
  };
})(window);

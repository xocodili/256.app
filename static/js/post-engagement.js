/**
 * Feed post engagement: vote gates participation; thread + compose unlock after voting.
 */
(function () {
  "use strict";

  var SPRITE = "/static/icons/post-actions.svg";

  function spriteIcon(symbolId, className, width, height) {
    return (
      '<svg class="' +
      className +
      '" viewBox="0 0 24 24" width="' +
      width +
      '" height="' +
      height +
      '" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><use href="' +
      SPRITE +
      "#" +
      symbolId +
      '"/></svg>'
    );
  }

  var THREAD_COUNT_ICON = spriteIcon("icon-thread-count", "post-engagement-count-icon", 20, 20);
  var REPLY_ICON = spriteIcon("icon-reply", "post-action-icon post-action-icon--reply", 20, 20);
  var OPEN_CASE_ICON = spriteIcon("icon-open-case", "post-action-icon post-action-icon--open-case", 20, 20);
  var VOTE_UP_ICON = spriteIcon("icon-vote-up", "post-action-icon post-action-icon--vote-up system-icon--md", 32, 32);
  var VOTE_DOWN_ICON = spriteIcon("icon-vote-down", "post-action-icon post-action-icon--vote-down system-icon--md", 32, 32);

  function ui() {
    try {
      var el = document.getElementById("home-shell-i18n");
      if (el && el.textContent) return JSON.parse(el.textContent);
    } catch (_) {}
    return {};
  }

  function t(key, fallback) {
    return ui()[key] || fallback;
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function feelLabel(feel) {
    if (feel === "loved") return t("post_engagement_feel_loved", "Loved");
    if (feel === "controversial") return t("post_engagement_feel_controversial", "Neutral");
    return t("post_engagement_feel_loathed", "Loathed");
  }

  function feelHeartSvg(feelKey) {
    var fill =
      feelKey === "loved"
        ? "var(--c-yellow)"
        : feelKey === "controversial"
          ? "var(--c-cyan)"
          : "var(--c-red)";
    return (
      '<svg class="post-engagement-feel-heart" viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" xmlns="http://www.w3.org/2000/svg"><use href="' +
      SPRITE +
      '#icon-feel-heart" fill="' +
      fill +
      '"/></svg>'
    );
  }

  async function postJson(url, body) {
    var r = await fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body || {}),
    });
    var data = await r.json().catch(function () {
      return {};
    });
    if (!r.ok) {
      throw new Error((data && data.error) || "Request failed");
    }
    return data;
  }

  function resolveProfileHandle(raw) {
    var HandleInput = window.AppHandleInput || {};
    var normalize =
      HandleInput.normalizeHandleCandidate ||
      function (s) {
        return String(s || "")
          .trim()
          .replace(/^@+/, "")
          .toLowerCase();
      };
    return normalize(String(raw || ""));
  }

  function profileUrl(handle) {
    var h = resolveProfileHandle(handle);
    return h ? "/home/profile/" + encodeURIComponent(h) : "#";
  }

  function profileLinkHtml(meta, innerHtml, className, ariaLabel) {
    var handle =
      meta && typeof meta === "object" ? String(meta.handle || "") : String(meta || "");
    var linkable =
      meta && typeof meta === "object" && meta.profile_linkable === false ? false : true;
    var href =
      meta && typeof meta === "object" && meta.profile_href
        ? String(meta.profile_href)
        : profileUrl(handle);
    var h = resolveProfileHandle(handle);
    if (!h || !linkable || !href || href === "#") return innerHtml;
    var label = ariaLabel || "Open profile @" + h;
    return (
      '<a class="feed-profile-link ' +
      (className || "") +
      '" href="' +
      escapeHtml(href) +
      '" data-profile-handle="' +
      escapeHtml(h) +
      '" aria-label="' +
      escapeHtml(label) +
      '">' +
      innerHtml +
      "</a>"
    );
  }

  function flagHtml(countryCode) {
    var cc = String(countryCode || "")
      .trim()
      .toLowerCase();
    if (cc.length !== 2 || !/^[a-z]{2}$/.test(cc)) return "";
    return (
      '<span class="public-post-flag-icon public-post-flag-inline fi fi-' +
      escapeHtml(cc) +
      '" data-country-code="' +
      escapeHtml(cc.toUpperCase()) +
      '" aria-hidden="true"></span>'
    );
  }

  function avatarHtml(c) {
    var inner;
    if (c.avatar_url) {
      inner =
        '<img class="post-thread-avatar-img" src="' +
        escapeHtml(c.avatar_url) +
        '" alt="" width="32" height="32" loading="lazy" decoding="async" referrerpolicy="no-referrer" />';
    } else {
      inner = '<span class="post-thread-avatar-img post-thread-avatar-img--fallback" aria-hidden="true">?</span>';
    }
    var framed =
      '<span class="brand-frame brand-frame--toolbar post-thread-avatar"><span class="avatar-stack">' +
      inner +
      "</span></span>";
    return profileLinkHtml(
      c,
      framed,
      "post-thread-avatar-link",
      "Open profile @" + String(c.handle || "")
    );
  }

  function replyComposeHtml(commentId) {
    var placeholder = escapeHtml(t("post_engagement_reply_placeholder", "Reply…"));
    return (
      '<div class="post-thread-reply-compose" hidden>' +
      '<input type="text" class="post-engagement-compose-input post-thread-reply-input" data-reply-parent="' +
      escapeHtml(String(commentId)) +
      '" placeholder="' +
      placeholder +
      '" autocomplete="off" />' +
      "</div>"
    );
  }

  function depthClass(depth) {
    if (depth === 2) return " post-thread-row--l2";
    if (depth >= 3) return " post-thread-row--l3";
    return "";
  }

  function renderCommentRow(c, readOnly) {
    if (c.is_hidden) {
      var hiddenLabel = escapeHtml(t("post_engagement_comment_hidden", "Comment hidden"));
      var depth = depthClass(c.depth || 1);
      var nested = "";
      if (c.children && c.children.length) {
        nested = c.children
          .map(function (child) {
            return renderCommentRow(child, readOnly);
          })
          .join("");
      }
      return (
        '<div class="post-thread-comment-block" data-comment-block="' +
        escapeHtml(String(c.id)) +
        '">' +
        '<article id="post-comment-' +
        escapeHtml(String(c.id)) +
        '" class="post-thread-row post-thread-row--hidden' +
        depth +
        '" data-comment-id="' +
        escapeHtml(String(c.id)) +
        '" data-comment-hidden="1">' +
        '<div class="post-thread-row-main">' +
        '<p class="post-thread-hidden-label">' +
        hiddenLabel +
        "</p>" +
        "</div></article>" +
        nested +
        "</div>"
      );
    }
    var mod = c.is_moderation_mirror ? " post-thread-row--mirror" : "";
    if (readOnly) mod += " post-thread-row--read-only";
    var depth = depthClass(c.depth || 1);
    var bodyId = "post-thread-body-" + String(c.id);
    var replyLabel = escapeHtml(t("post_engagement_reply", "Reply"));
    var openCaseLabel = escapeHtml(t("post_engagement_open_case", "Open case on this post"));
    var showMore = escapeHtml(t("post_engagement_show_more", "Show more …"));
    var voteAria = escapeHtml(t("post_engagement_vote_aria", "Vote on this post"));
    var actions = "";
    if (!readOnly && !c.is_moderation_mirror) {
      if (c.can_vote !== false) {
        var viewerVote = c.viewer_vote || "";
        actions +=
          '<span class="post-thread-vote-group" data-comment-vote-group="' +
          escapeHtml(String(c.id)) +
          '" aria-label="' +
          voteAria +
          '">' +
          '<button type="button" class="post-action-btn post-thread-action-btn post-thread-vote-btn post-thread-vote-btn--yes' +
          (viewerVote === "yes" ? " is-active" : "") +
          '" data-comment-vote="yes" data-comment-id="' +
          escapeHtml(String(c.id)) +
          '" title="' +
          voteAria +
          '">' +
          VOTE_UP_ICON +
          "</button>" +
          '<button type="button" class="post-action-btn post-thread-action-btn post-thread-vote-btn post-thread-vote-btn--no' +
          (viewerVote === "no" ? " is-active" : "") +
          '" data-comment-vote="no" data-comment-id="' +
          escapeHtml(String(c.id)) +
          '" title="' +
          voteAria +
          '">' +
          VOTE_DOWN_ICON +
          "</button></span>";
      }
      actions +=
        '<a class="post-thread-action-btn post-thread-open-case-btn" href="/home/judicial/post/' +
        escapeHtml(String(c.id)) +
        '?return_to=%2Fhome" title="' +
        openCaseLabel +
        '" aria-label="' +
        openCaseLabel +
        '">' +
        OPEN_CASE_ICON +
        "</a>";
      if (c.can_reply !== false) {
        actions +=
          '<button type="button" class="post-thread-action-btn post-thread-reply-btn" data-reply-to="' +
          escapeHtml(String(c.id)) +
          '" title="' +
          replyLabel +
          '" aria-label="' +
          replyLabel +
          '">' +
          REPLY_ICON +
          "</button>";
      }
    }
    var nested = "";
    if (!readOnly && c.children && c.children.length) {
      nested = c.children.map(function (child) {
        return renderCommentRow(child, readOnly);
      }).join("");
    }
    return (
      '<div class="post-thread-comment-block" data-comment-block="' +
      escapeHtml(String(c.id)) +
      '">' +
      '<article id="post-comment-' +
      escapeHtml(String(c.id)) +
      '" class="post-thread-row' +
      mod +
      depth +
      '" data-comment-id="' +
      escapeHtml(String(c.id)) +
      '">' +
      avatarHtml(c) +
      '<div class="post-thread-row-main">' +
      '<div class="post-thread-head">' +
      profileLinkHtml(
        c,
        "@" + escapeHtml(c.handle),
        "public-post-handle post-thread-handle",
        "Open profile @" + String(c.handle || "")
      ) +
      flagHtml(c.country_code) +
      "</div>" +
      '<div id="' +
      bodyId +
      '" class="post-thread-body post-thread-body--clamp">' +
      (c.body_html || "") +
      "</div>" +
      '<button type="button" class="post-thread-expand-btn" data-thread-expand-target="' +
      bodyId +
      '" hidden>' +
      showMore +
      "</button>" +
      "</div>" +
      '<div class="post-thread-row-actions">' +
      actions +
      "</div></article>" +
      (c.can_reply !== false && !c.is_moderation_mirror && !readOnly ? replyComposeHtml(c.id) : "") +
      nested +
      "</div>"
    );
  }

  function renderComment(c, readOnly) {
    return renderCommentRow(c, readOnly);
  }

  function previewVotePromptHtml() {
    var label = escapeHtml(
      t("post_engagement_vote_to_comment_reply", "Vote to comment & reply")
    );
    return '<p class="post-thread-preview-cta" role="note">' + label + "</p>";
  }

  function renderCommentsHtml(comments, readOnly) {
    var parts = [];
    (comments || []).forEach(function (c) {
      parts.push(renderComment(c, readOnly));
    });
    return parts.join("");
  }

  function bindThreadExpand(scope) {
    (scope || document).querySelectorAll(".post-thread-body--clamp").forEach(function (body) {
      var btn = body.parentElement
        ? body.parentElement.querySelector("[data-thread-expand-target]")
        : null;
      if (!btn) return;
      if (body.scrollHeight > body.clientHeight + 2) {
        btn.hidden = false;
      }
    });
  }

  function closeAllReplyComposes(card) {
    card.querySelectorAll(".post-thread-reply-compose").forEach(function (el) {
      el.hidden = true;
    });
  }

  function bindComposeMentionTypeahead(input, card) {
    if (!input || !window.AppHandleTypeahead || input.getAttribute("data-mention-typeahead") === "1") {
      return;
    }
    input.setAttribute("data-mention-typeahead", "1");
    window.AppHandleTypeahead.bindInput({
      input: input,
      mode: "mention",
      trigger: "at",
      overlayLimit: 3,
      showAllLabel: t("mention_show_all", "Show all ({count})"),
      ariaLabel: t("post_engagement_mention_suggestions", "Mention suggestions"),
      getContext: function () {
        return {
          entityId:
            card.getAttribute("data-viewer-entity-id") ||
            card.getAttribute("data-post-entity-id"),
          rootPostId: card.getAttribute("data-post-id") || "",
          audienceHandles: card.getAttribute("data-post-audience-handles") || "",
        };
      },
      fallbackGlobal: false,
    });
  }

  function bindComposeInputsInCard(card) {
    if (!card) return;
    card.querySelectorAll(".post-engagement-compose-input, .post-thread-reply-input").forEach(
      function (input) {
        bindComposeMentionTypeahead(input, card);
      }
    );
  }

  function openReplyCompose(card, commentId) {
    closeAllReplyComposes(card);
    var block = card.querySelector('[data-comment-block="' + commentId + '"]');
    if (!block) return;
    var compose = block.querySelector(".post-thread-reply-compose");
    if (!compose) return;
    compose.hidden = false;
    var input = compose.querySelector("input");
    if (input) {
      bindComposeMentionTypeahead(input, card);
      input.focus();
    }
  }

  function loadMoreButtonHtml(rootId, nextOffset) {
    var label = escapeHtml(t("post_engagement_load_more", "Load more comments"));
    return (
      '<button type="button" class="post-thread-load-more" data-thread-load-more="' +
      escapeHtml(String(rootId)) +
      '" data-l1-offset="' +
      escapeHtml(String(nextOffset)) +
      '">' +
      label +
      "</button>"
    );
  }

  function renderThread(slot, data, append) {
    if (data.preview_only) {
      var previewParts = [];
      if (data.comments && data.comments.length) {
        previewParts.push('<div class="post-thread-comments post-thread-comments--preview">');
        previewParts.push(renderCommentsHtml(data.comments, true));
        previewParts.push("</div>");
      }
      previewParts.push(previewVotePromptHtml());
      slot.innerHTML = previewParts.join("");
      slot.hidden = false;
      bindThreadExpand(slot);
      return;
    }
    if (!data.can_view_discussion) {
      slot.innerHTML = "";
      slot.hidden = true;
      return;
    }
    if (!append) {
      var parts = [];
      if (data.mirrors && data.mirrors.length) {
        parts.push('<div class="post-thread-mirrors">');
        data.mirrors.forEach(function (m) {
          parts.push(renderComment(m, false));
        });
        parts.push("</div>");
      }
      parts.push('<div class="post-thread-comments">');
      parts.push(renderCommentsHtml(data.comments, false));
      parts.push("</div>");
      slot.innerHTML = parts.join("");
    } else {
      var list = slot.querySelector(".post-thread-comments");
      if (list && data.comments && data.comments.length) {
        list.insertAdjacentHTML("beforeend", renderCommentsHtml(data.comments, false));
      }
      var oldBtn = slot.querySelector(".post-thread-load-more");
      if (oldBtn) oldBtn.remove();
    }
    if (data.has_more_l1 && data.l1_next_offset != null) {
      slot.insertAdjacentHTML("beforeend", loadMoreButtonHtml(data.root_post_id, data.l1_next_offset));
    }
    slot.setAttribute("data-l1-offset", String(data.l1_next_offset != null ? data.l1_next_offset : data.l1_offset || 0));
    slot.hidden = slot.innerHTML.trim().length === 0;
    bindThreadExpand(slot);
  }

  async function loadThread(rootId, slot, l1Offset, append) {
    var url =
      "/posts/" +
      encodeURIComponent(String(rootId)) +
      "/thread?l1_offset=" +
      encodeURIComponent(String(l1Offset || 0));
    var r = await fetch(url, {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
    });
    if (!r.ok) return;
    var data = await r.json();
    renderThread(slot, data, append);
  }

  function bindVoteButtons(card, rootId) {
    card.querySelectorAll("[data-vote]").forEach(function (btn) {
      if (btn.getAttribute("data-vote-bound") === "1") return;
      btn.setAttribute("data-vote-bound", "1");
      btn.addEventListener("click", function () {
        onVote(card, rootId, btn.getAttribute("data-vote")).catch(function (e) {
          window.alert(e.message || "Could not record vote.");
        });
      });
    });
  }

  function refreshEngagementBar(card, data) {
    var bar = card.querySelector("[data-post-engagement-bar]");
    if (!bar || !data) return;
    var rootId = bar.getAttribute("data-root-id");
    var feelKey = data.feel || "controversial";
    var feelText = feelLabel(feelKey);
    var placeholder = escapeHtml(t("post_engagement_comment_placeholder", "Comment on this post"));
    bar.className = "post-engagement-bar post-engagement-bar--voted";
    bar.setAttribute("data-voted", "1");
    bar.setAttribute("data-feel", feelKey);
    bar.innerHTML =
      '<label class="post-engagement-compose-wrap"><span class="visually-hidden">' +
      placeholder +
      '</span><input type="text" class="post-engagement-compose-input" data-post-comment-root="' +
      escapeHtml(rootId) +
      '" placeholder="' +
      placeholder +
      '" autocomplete="off" /></label>' +
      '<span class="post-engagement-feel-badge post-engagement-feel-badge--mood post-engagement-feel--' +
      escapeHtml(feelKey) +
      '" title="' +
      escapeHtml(feelText) +
      '" aria-label="' +
      escapeHtml(feelText) +
      '">' +
      feelHeartSvg(feelKey) +
      "</span>";

    var slot = card.querySelector("[data-thread-slot]");
    if (slot) {
      slot.hidden = false;
    }
    bindComposeInputsInCard(card);
  }

  async function onCommentVote(card, rootId, commentId, direction) {
    var data = await postJson(
      "/posts/" +
        encodeURIComponent(String(rootId)) +
        "/comments/" +
        encodeURIComponent(String(commentId)) +
        "/engagement-vote",
      { direction: direction }
    );
    var group = card.querySelector('[data-comment-vote-group="' + commentId + '"]');
    if (group) {
      group.querySelectorAll("[data-comment-vote]").forEach(function (btn) {
        btn.classList.toggle("is-active", btn.getAttribute("data-comment-vote") === data.direction);
      });
    }
  }

  async function onVote(card, rootId, direction) {
    var data = await postJson(
      "/posts/" + encodeURIComponent(String(rootId)) + "/engagement-vote",
      { direction: direction }
    );
    refreshEngagementBar(card, data);
    var slot = card.querySelector("[data-thread-slot]");
    if (slot) await loadThread(rootId, slot, 0, false);
  }

  async function onComment(card, rootId, parentId, text, entityId) {
    if (!text || !text.trim()) return;
    await postJson("/posts/" + encodeURIComponent(String(rootId)) + "/comments", {
      entity_id: String(entityId),
      parent_post_id: String(parentId),
      body_markup: "<p>" + escapeHtml(text.trim()) + "</p>",
    });
    closeAllReplyComposes(card);
    var slot = card.querySelector("[data-thread-slot]");
    if (slot) await loadThread(rootId, slot, 0, false);
  }

  async function onShareLink(entityId, linkId, audience) {
    await postJson("/posts/share-link", {
      entity_id: String(entityId),
      content_link_id: String(linkId),
      audience_handles: audience,
    });
    if (typeof window.showHomeFeedPanel === "function") {
      window.showHomeFeedPanel();
    } else {
      window.location.reload();
    }
  }

  function submitComposeInput(card, rootId, input, entityId) {
    var parentId =
      input.getAttribute("data-reply-parent") ||
      input.getAttribute("data-post-comment-root") ||
      rootId;
    onComment(card, rootId, parentId, input.value, entityId).catch(function (err) {
      window.alert(err.message || "Could not post comment.");
    });
    input.value = "";
  }

  function bindCard(card) {
    if (!card || card.getAttribute("data-engagement-bound") === "1") return;
    try {
      bindCardInner(card);
    } catch (err) {
      console.error(
        "post engagement bind failed",
        card.getAttribute("data-post-id"),
        err
      );
    }
  }

  function bindCardInner(card) {
    card.setAttribute("data-engagement-bound", "1");
    var rootId = card.getAttribute("data-post-id");
    if (!rootId) return;
    var entityId =
      card.getAttribute("data-viewer-entity-id") || card.getAttribute("data-post-entity-id");
    var bar = card.querySelector("[data-post-engagement-bar]");
    var slot = card.querySelector("[data-thread-slot]");
    var voted = bar && bar.getAttribute("data-voted") === "1";

    bindVoteButtons(card, rootId);

    if (slot) {
      loadThread(rootId, slot, 0, false).catch(function () {});
    }

    bindComposeInputsInCard(card);

    card.addEventListener("keydown", function (e) {
      if (e.key !== "Enter") return;
      var input = e.target;
      if (
        !input ||
        (!input.classList.contains("post-engagement-compose-input") &&
          !input.classList.contains("post-thread-reply-input"))
      ) {
        return;
      }
      e.preventDefault();
      submitComposeInput(card, rootId, input, entityId);
    });

    card.addEventListener("click", function (e) {
      var expandBtn =
        e.target && e.target.closest ? e.target.closest(".post-thread-expand-btn") : null;
      if (expandBtn) {
        var targetId = expandBtn.getAttribute("data-thread-expand-target") || "";
        var line = targetId ? document.getElementById(targetId) : null;
        if (line) {
          line.classList.remove("post-thread-body--clamp");
          expandBtn.hidden = true;
        }
        return;
      }

      var replyBtn = e.target && e.target.closest ? e.target.closest(".post-thread-reply-btn") : null;
      if (replyBtn) {
        e.preventDefault();
        var parent = replyBtn.getAttribute("data-reply-to") || rootId;
        openReplyCompose(card, parent);
        return;
      }

      var commentVoteBtn =
        e.target && e.target.closest ? e.target.closest("[data-comment-vote]") : null;
      if (commentVoteBtn) {
        e.preventDefault();
        var commentId = commentVoteBtn.getAttribute("data-comment-id");
        var dir = commentVoteBtn.getAttribute("data-comment-vote");
        if (commentId && dir) {
          onCommentVote(card, rootId, commentId, dir).catch(function (err) {
            window.alert(err.message || "Could not record vote.");
          });
        }
        return;
      }

      var loadMoreBtn =
        e.target && e.target.closest ? e.target.closest(".post-thread-load-more") : null;
      if (loadMoreBtn) {
        e.preventDefault();
        var slot = card.querySelector("[data-thread-slot]");
        var nextOffset = loadMoreBtn.getAttribute("data-l1-offset") || "0";
        if (slot) {
          loadThread(rootId, slot, parseInt(nextOffset, 10) || 0, true).catch(function () {});
        }
        return;
      }

      var shareBtn =
        e.target && e.target.closest ? e.target.closest(".post-share-audience-btn") : null;
      if (shareBtn) {
        var linkId = shareBtn.getAttribute("data-share-link-id");
        var ent = shareBtn.getAttribute("data-share-entity-id") || entityId;
        var audience = window.prompt(
          t("post_engagement_share_audience_prompt", "For: @friends, @private"),
          t("post_engagement_share_audience_prompt", "For: @friends, @private")
        );
        if (audience) {
          onShareLink(ent, linkId, audience).catch(function (err) {
            window.alert(err.message || "Could not share link.");
          });
        }
      }
    });
  }

  window.bindPostEngagementFeed = function (root) {
    var scope = root || document;
    scope.querySelectorAll(".public-post-card[data-post-id]").forEach(bindCard);
  };
})();

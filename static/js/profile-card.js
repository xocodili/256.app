/**
 * Private profile card — mount, bind, and mutation handlers (root-scoped).
 */
(function (global) {
  "use strict";

  function csrfToken() {
    var meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? String(meta.getAttribute("content") || "").trim() : "";
  }

  function mutationHeaders(extra) {
    var h = { Accept: "application/json" };
    var token = csrfToken();
    if (token) h["X-CSRF-Token"] = token;
    if (extra) {
      for (var k in extra) {
        if (Object.prototype.hasOwnProperty.call(extra, k)) h[k] = extra[k];
      }
    }
    return h;
  }

  function byId(root, id) {
    return root ? root.querySelector("#" + id) : null;
  }

  function dismissProfileCard(deps) {
    var slot = document.getElementById("app-private-profile-card-slot");
    if (slot && slot.parentNode) slot.parentNode.removeChild(slot);
    if (deps && typeof deps.onDismiss === "function") deps.onDismiss();
    if (deps && typeof deps.showHomeFeedPanel === "function") deps.showHomeFeedPanel();
  }

  function showConnectNotice(root, notice, deps) {
    var block = byId(root, "profile-connect-block");
    if (block) {
      block.innerHTML =
        '<p class="profile-value" id="profile-connect-sent-notice">' +
        deps.escapeHtml(String(notice)) +
        "</p>";
    }
  }

  function isVisibleForMeasure(el) {
    if (!el) return false;
    var node = el;
    while (node) {
      if (node.hidden) return false;
      node = node.parentElement;
    }
    return el.getClientRects().length > 0;
  }

  function resetClampedExpander(root, bodyId, btnSelector, boundAttr) {
    var body = root.querySelector(bodyId);
    var btn = root.querySelector(btnSelector);
    if (body) {
      body.classList.add("is-clamped");
      body.classList.remove("is-expanded");
    }
    if (btn) {
      btn.hidden = true;
      btn.setAttribute("aria-expanded", "false");
    }
    root.removeAttribute(boundAttr);
  }

  function bindConstitutionExpander(root, UI) {
    if (!root || root.getAttribute("data-constitution-bound") === "1") return;
    var body = root.querySelector("#profile-constitution-body");
    var btn = root.querySelector("[data-profile-constitution-expand]");
    if (!body || !btn) return;
    root.setAttribute("data-constitution-bound", "1");
    body.classList.add("is-clamped");
    var showMore = (UI && UI.constitution_show_more) || "Show more …";
    var showLess = (UI && UI.constitution_show_less) || "Show less";
    function syncButton() {
      if (!isVisibleForMeasure(body)) {
        body.classList.add("is-clamped");
        btn.hidden = true;
        btn.setAttribute("aria-expanded", "false");
        btn.textContent = showMore;
        return;
      }
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
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(syncButton);
  }

  function bindAboutExpander(root, UI) {
    if (!root || root.getAttribute("data-about-bound") === "1") return;
    var body = root.querySelector("#profile-about-body");
    var btn = root.querySelector("[data-profile-about-expand]");
    if (!body || !btn) return;
    root.setAttribute("data-about-bound", "1");
    body.classList.add("is-clamped");
    var showMore = (UI && UI.constitution_show_more) || "Show more …";
    var showLess = (UI && UI.constitution_show_less) || "Show less";
    function syncButton() {
      if (!isVisibleForMeasure(body)) {
        body.classList.add("is-clamped");
        btn.hidden = true;
        btn.setAttribute("aria-expanded", "false");
        btn.textContent = showMore;
        return;
      }
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
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(syncButton);
  }

  function profileViewToggleRows(root) {
    return {
      rowView: root.querySelector(".profile-view-toggle-view"),
      rowBackTop: root.querySelector(".profile-view-toggle-back"),
      rowBackBottom: root.querySelector(".profile-view-full-row--bottom"),
    };
  }

  function setProfileViewToggleRows(rows, mode) {
    if (mode === "hidden") {
      if (rows.rowView) rows.rowView.hidden = true;
      if (rows.rowBackTop) rows.rowBackTop.hidden = true;
      if (rows.rowBackBottom) rows.rowBackBottom.hidden = true;
      return;
    }
    var isSummary = mode === "summary";
    if (rows.rowView) rows.rowView.hidden = !isSummary;
    if (rows.rowBackTop) rows.rowBackTop.hidden = isSummary;
    if (rows.rowBackBottom) rows.rowBackBottom.hidden = isSummary;
    var btnView = rows.rowView ? rows.rowView.querySelector("[data-profile-view-toggle]") : null;
    var btnBackTop = rows.rowBackTop ? rows.rowBackTop.querySelector("[data-profile-view-toggle]") : null;
    var btnBackBottom = rows.rowBackBottom
      ? rows.rowBackBottom.querySelector("[data-profile-view-toggle]")
      : null;
    var expanded = isSummary ? "false" : "true";
    if (btnView) btnView.setAttribute("aria-expanded", isSummary ? "false" : "true");
    if (btnBackTop) btnBackTop.setAttribute("aria-expanded", expanded);
    if (btnBackBottom) btnBackBottom.setAttribute("aria-expanded", expanded);
  }

  function bindProfileViewToggle(root, deps) {
    if (!root || root.getAttribute("data-view-toggle-bound") === "1") return;
    var rows = profileViewToggleRows(root);
    var btnView = rows.rowView ? rows.rowView.querySelector("[data-profile-view-toggle]") : null;
    var btnBackTop = rows.rowBackTop ? rows.rowBackTop.querySelector("[data-profile-view-toggle]") : null;
    var btnBackBottom = rows.rowBackBottom
      ? rows.rowBackBottom.querySelector("[data-profile-view-toggle]")
      : null;
    var details = byId(root, "profile-entity-detail-sections");
    if ((!btnView && !btnBackBottom) || !details) return;
    root.setAttribute("data-view-toggle-bound", "1");

    function showSummary() {
      details.hidden = true;
      root.setAttribute("data-profile-view", "summary");
      setProfileViewToggleRows(rows, "summary");
      resetClampedExpander(root, "#profile-about-body", "[data-profile-about-expand]", "data-about-bound");
      bindAboutExpander(root, (deps && deps.UI) || {});
      if (deps && typeof deps.scrollFeedToTop === "function") deps.scrollFeedToTop();
    }

    function showFull() {
      details.hidden = false;
      root.removeAttribute("data-profile-view");
      setProfileViewToggleRows(rows, "full");
      resetClampedExpander(root, "#profile-constitution-body", "[data-profile-constitution-expand]", "data-constitution-bound");
      var UI = (deps && deps.UI) || {};
      bindConstitutionExpander(root, UI);
      bindAboutExpander(root, UI);
      if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(function () {
          bindConstitutionExpander(root, UI);
          bindAboutExpander(root, UI);
        });
      }
      if (deps && typeof deps.scrollFeedToTop === "function") deps.scrollFeedToTop();
    }

    function onToggleClick() {
      if (details.hidden) showFull();
      else showSummary();
    }

    if (btnView) btnView.addEventListener("click", onToggleClick);
    if (btnBackTop) btnBackTop.addEventListener("click", onToggleClick);
    if (btnBackBottom) btnBackBottom.addEventListener("click", onToggleClick);

    if (root.getAttribute("data-profile-summary-only") === "1") {
      showSummary();
    }
  }

  function bindConnectForm(root, deps) {
    var form = byId(root, "profile-connect-form");
    var select = byId(root, "profile-connect-as");
    if (!form || !select || form.getAttribute("data-connect-bound") === "1") return;
    form.setAttribute("data-connect-bound", "1");
    var isMembershipReview = form.getAttribute("data-membership-review") === "1";
    var editor = byId(root, "profile-membership-review-editor");
    var statementInput = byId(root, "profile-membership-review-statement-input");
    var toolbar = form.querySelector(".create-post-editor-toolbar");
    if (toolbar && editor) {
      toolbar.addEventListener("click", function (e) {
        var btn = e.target && e.target.closest ? e.target.closest("[data-cmd]") : null;
        if (!btn) return;
        e.preventDefault();
        var cmd = btn.getAttribute("data-cmd");
        if (!cmd) return;
        editor.focus();
        var value = btn.getAttribute("data-value");
        document.execCommand(cmd, false, value != null ? value : null);
      });
    }
    select.addEventListener("change", async function () {
      var connectAs = (select.value || "").trim().toLowerCase();
      if (!connectAs) return;
      if (isMembershipReview && connectAs === "member") {
        var markup = editor ? String(editor.innerHTML || "").trim() : "";
        var plain = editor
          ? String(editor.innerText || editor.textContent || "")
              .replace(/\r\n/g, "\n")
              .trim()
          : "";
        if (!plain) {
          deps.notify(
            deps.UI.membership_review_statement_required ||
              "Write a short statement for the community jury.",
            3200
          );
          select.value = "";
          return;
        }
        if (statementInput) statementInput.value = markup;
      }
      select.disabled = true;
      try {
        var fd = new URLSearchParams();
        fd.set("connect_as", connectAs);
        if (isMembershipReview && statementInput) {
          fd.set("application_statement", statementInput.value || "");
        }
        var r = await fetch(form.getAttribute("action"), {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/x-www-form-urlencoded" }),
          body: fd.toString(),
        });
        var payload = await r.json().catch(function () {
          return {};
        });
        if (!r.ok) {
          throw new Error(
            (payload && (payload.error || payload.message)) ||
              deps.UI.connect_send_failed ||
              "Could not send invitation right now"
          );
        }
        var notice =
          connectAs === "block"
            ? (payload && payload.message) || deps.UI.block_success || "Blocked."
            : (payload && payload.message) ||
              deps.UI.connect_invitation_sent ||
              "Connection Invitation Sent";
        if (connectAs === "block") {
          dismissProfileCard(deps);
          deps.notify(notice, 3000);
          return;
        }
        showConnectNotice(root, notice, deps);
        deps.notify(notice, 3000);
      } catch (e) {
        deps.notify(
          (e && e.message) || deps.UI.connect_send_failed || "Could not send invitation right now",
          3000
        );
        select.value = "";
        select.disabled = false;
      }
    });
  }

  function bindInviteResponseForm(root, deps) {
    var form = byId(root, "profile-invite-response-form");
    var select = byId(root, "profile-invite-response");
    if (!form || !select || form.getAttribute("data-invite-response-bound") === "1") return;
    form.setAttribute("data-invite-response-bound", "1");
    select.addEventListener("change", async function () {
      var responseAction = (select.value || "").trim().toLowerCase();
      if (!responseAction) return;
      select.disabled = true;
      try {
        var fd = new URLSearchParams();
        fd.set("response_action", responseAction);
        var r = await fetch(form.getAttribute("action"), {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/x-www-form-urlencoded" }),
          body: fd.toString(),
        });
        if (!r.ok) throw new Error("invite_response_failed");
        var msg =
          responseAction === "accept"
            ? deps.UI.invite_response_accepted
            : responseAction === "decline"
              ? deps.UI.invite_response_declined
              : deps.UI.invite_response_blocked;
        if (responseAction === "block") {
          dismissProfileCard(deps);
          deps.notify(msg || deps.UI.block_success || "Blocked.", 3000);
          return;
        }
        showConnectNotice(root, msg || "", deps);
      } catch (_e) {
        deps.notify(
          deps.UI.connect_send_failed || "Could not update connection right now",
          3000
        );
        select.value = "";
        select.disabled = false;
      }
    });
  }

  function bindBlockForm(root, deps) {
    var form = byId(root, "profile-block-form");
    if (!form || form.getAttribute("data-block-bound") === "1") return;
    form.setAttribute("data-block-bound", "1");
    form.addEventListener("submit", async function (e) {
      e.preventDefault();
      var btn = form.querySelector("button[type='submit']");
      if (btn) btn.disabled = true;
      try {
        var r = await fetch(form.getAttribute("action"), {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders(),
        });
        var payload = await r.json().catch(function () {
          return {};
        });
        if (!r.ok) {
          throw new Error(
            (payload && (payload.error || payload.message)) || "block_failed"
          );
        }
        var msg = (payload && payload.message) || deps.UI.block_success || "Blocked.";
        dismissProfileCard(deps);
        deps.notify(msg, 3000);
      } catch (_e) {
        deps.notify(
          deps.UI.connect_send_failed || "Could not block that profile right now",
          3000
        );
        if (btn) btn.disabled = false;
      }
    });
  }

  function bindSimplePostForm(root, formId, deps, onSuccess) {
    var form = byId(root, formId);
    if (!form || form.getAttribute("data-bound") === "1") return;
    form.setAttribute("data-bound", "1");
    form.addEventListener("submit", async function (e) {
      e.preventDefault();
      var btn = form.querySelector("button[type='submit']");
      if (btn) btn.disabled = true;
      try {
        var r = await fetch(form.getAttribute("action"), {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders(),
        });
        if (!r.ok) throw new Error("submit_failed");
        if (onSuccess) onSuccess(form);
      } catch (_e) {
        deps.notify(deps.UI.connect_send_failed || "Could not complete that action right now", 3000);
        if (btn) btn.disabled = false;
      }
    });
  }

  function bindGatheringRsvpForm(root, deps) {
    var form = byId(root, "profile-gathering-rsvp-form");
    var select = byId(root, "profile-gathering-rsvp");
    if (!form || !select || form.getAttribute("data-gathering-rsvp-bound") === "1") return;
    form.setAttribute("data-gathering-rsvp-bound", "1");
    select.addEventListener("change", async function () {
      var v = (select.value || "").trim().toLowerCase();
      if (!v) return;
      select.disabled = true;
      try {
        var fd = new URLSearchParams();
        fd.set("gathering_rsvp", v);
        var r = await fetch(form.getAttribute("action"), {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/x-www-form-urlencoded" }),
          body: fd.toString(),
        });
        if (!r.ok) throw new Error("gathering_rsvp_failed");
        deps.notify(deps.UI.gathering_rsvp_saved_notice || "Saved", 2600);
      } catch (_e) {
        deps.notify(deps.UI.connect_send_failed || "Could not save RSVP right now", 3000);
        select.value = "";
      } finally {
        select.disabled = false;
      }
    });
  }

  function bindHumanEditForm(root, deps) {
    if (!root || root.getAttribute("data-profile-human-edit-bound") === "1") return;
    var toggle = byId(root, "profile-human-edit-toggle");
    var block = byId(root, "profile-human-edit-block");
    var form = byId(root, "profile-human-edit-form");
    var cancelBtn = byId(root, "profile-human-edit-cancel");
    var msg = byId(root, "profile-human-edit-msg");
    if (!toggle || !block || !form) return;

    var profileAccountPanels = global.ProfileAccount
      ? global.ProfileAccount.bind({
          root: root,
          block: block,
          editPanel: byId(root, "profile-human-edit-panel"),
          passwordBlock: byId(root, "profile-change-password-block"),
          passwordVerifyStep: byId(root, "profile-change-password-step-verify"),
          passwordNewStep: byId(root, "profile-change-password-step-new"),
          passwordMsg: byId(root, "profile-change-password-msg"),
          blocksBlock: byId(root, "profile-manage-blocks-block"),
          blocksList: byId(root, "profile-block-list"),
          blocksEmpty: byId(root, "profile-block-list-empty"),
          blocksMsg: byId(root, "profile-block-list-msg"),
          deleteBlock: byId(root, "profile-delete-account-block"),
          deleteMsg: byId(root, "profile-delete-account-msg"),
          scrollFeedToTop: deps.scrollFeedToTop,
          notify: deps.notify,
          UI: deps.UI,
          mutationHeaders: mutationHeaders,
        })
      : null;

    function clearMsg(el) {
      if (!el) return;
      el.hidden = true;
      el.textContent = "";
    }
    function showMsg(el, text) {
      if (!el) return;
      el.hidden = !text;
      el.textContent = text || "";
    }
    function resetEditPanels() {
      if (profileAccountPanels && profileAccountPanels.resetAccountPanels) {
        profileAccountPanels.resetAccountPanels();
      }
      clearMsg(msg);
    }
    function restoreProfileViewRows() {
      var isSummary = viewBeforeEdit === "summary";
      if (viewBeforeEdit === null) {
        isSummary = root.getAttribute("data-profile-view") === "summary";
      }
      var rows = profileViewToggleRows(root);
      var detailSections = byId(root, "profile-entity-detail-sections");
      if (isSummary) {
        if (detailSections) detailSections.hidden = true;
        root.setAttribute("data-profile-view", "summary");
        setProfileViewToggleRows(rows, "summary");
      } else {
        if (detailSections) detailSections.hidden = false;
        root.removeAttribute("data-profile-view");
        setProfileViewToggleRows(rows, "full");
      }
    }
    var viewBeforeEdit = null;
    function setEditOpen(open) {
      block.hidden = !open;
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
      var rows = profileViewToggleRows(root);
      var detailSections = byId(root, "profile-entity-detail-sections");
      var logoutAction = byId(root, "profile-logout-action");
      if (open) {
        viewBeforeEdit =
          root.getAttribute("data-profile-view") === "summary" || !detailSections || detailSections.hidden
            ? "summary"
            : "full";
        if (detailSections) detailSections.hidden = true;
        setProfileViewToggleRows(rows, "hidden");
        if (logoutAction) logoutAction.hidden = true;
      } else {
        restoreProfileViewRows();
        viewBeforeEdit = null;
        if (logoutAction) logoutAction.hidden = false;
        if (deps && typeof deps.scrollFeedToTop === "function") deps.scrollFeedToTop();
      }
      if (!open) resetEditPanels();
      else clearMsg(msg);
    }
    toggle.addEventListener("click", function () {
      if (block.hidden) resetEditPanels();
      setEditOpen(block.hidden);
    });
    if (cancelBtn) {
      cancelBtn.addEventListener("click", function () {
        setEditOpen(false);
      });
    }

    form.addEventListener("submit", async function (e) {
      e.preventDefault();
      var saveBtn = byId(root, "profile-human-edit-save");
      if (saveBtn) saveBtn.disabled = true;
      clearMsg(msg);
      try {
        var fd = new FormData(form);
        var draft = {
          given_name: (fd.get("given_name") || "").toString().trim(),
          family_name: (fd.get("family_name") || "").toString().trim(),
          additional_name: (fd.get("additional_name") || "").toString().trim() || null,
          country_code: (fd.get("country_code") || "").toString().trim().toUpperCase(),
          region: (fd.get("region") || "").toString().trim() || null,
          locality: (fd.get("locality") || "").toString().trim() || null,
          public_location_label: (fd.get("public_location_label") || "").toString().trim() || null,
          website_url: (fd.get("website_url") || "").toString().trim() || null,
        };
        var r = await fetch("/me/human-profile", {
          method: "PATCH",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/json" }),
          body: JSON.stringify(draft),
        });
        var payload = await r.json().catch(function () { return {}; });
        if (!r.ok) {
          throw new Error(
            (payload && payload.error) ||
              deps.UI.profile_edit_failed ||
              "Could not update profile right now"
          );
        }
        deps.notify(deps.UI.profile_edit_success || "Profile updated.", 3200);
        setEditOpen(false);
        if (deps.currentProfileHandle) {
          deps.openPrivateProfileCard(deps.currentProfileHandle, false, true);
          deps.scrollFeedToTop();
        }
      } catch (err) {
        showMsg(
          msg,
          (err && err.message) || deps.UI.profile_edit_failed || "Could not update profile right now"
        );
        deps.notify(
          (err && err.message) || deps.UI.profile_edit_failed || "Could not update profile right now",
          4000
        );
      } finally {
        if (saveBtn) saveBtn.disabled = false;
      }
    });

    root.setAttribute("data-profile-human-edit-bound", "1");
  }

  var EntityFormCommon = global.CreateEntityFormCommon || {};

  function bindEntityEditDescription(root) {
    var form = byId(root, "profile-entity-edit-form");
    if (!EntityFormCommon.bindRichDescription) return;
    EntityFormCommon.bindRichDescription({
      descriptionInput: byId(root, "profile-entity-edit-description"),
      descriptionEditor: byId(root, "profile-entity-edit-description-editor"),
      descriptionInitial: byId(root, "profile-entity-edit-description-initial"),
      descriptionToolbar: form ? form.querySelector(".create-entity-description-toolbar") : null,
      descriptionCharHint: byId(root, "profile-entity-edit-description-char-hint"),
      maxChars: EntityFormCommon.DEFAULT_DESC_MAX || 4000,
    });
  }

  function bindEntityEditPlaceSearch(root, deps) {
    if (!EntityFormCommon.bindGooglePlaceSearch) return;
    EntityFormCommon.bindGooglePlaceSearch({
      locInput: byId(root, "profile-entity-edit-location-id"),
      placeQueryInput: byId(root, "profile-entity-edit-google-place-query"),
      placeResults: byId(root, "profile-entity-edit-place-results"),
      placeBadge: byId(root, "profile-entity-edit-place-badge"),
      placeIdInput: byId(root, "profile-entity-edit-google-place-id"),
      UI: deps.UI || {},
      escapeHtml: deps.escapeHtml || function (s) {
        return String(s || "");
      },
      buildHeaders: function (extra) {
        return mutationHeaders(extra);
      },
      clearLocationOnQueryChange: false,
      initialShowReadyIfSet: true,
    });
  }

  function bindEntityEditOwnerHandles(root) {
    if (!EntityFormCommon.bindOwnerHandles) return;
    EntityFormCommon.bindOwnerHandles(byId(root, "profile-entity-edit-owner-handles"));
  }

  function bindEntityEditForm(root, deps) {
    if (!root || root.getAttribute("data-profile-entity-edit-bound") === "1") return;
    var toggle = byId(root, "profile-entity-edit-toggle");
    var block = byId(root, "profile-entity-edit-block");
    var form = byId(root, "profile-entity-edit-form");
    var cancelBtn = byId(root, "profile-entity-edit-cancel");
    var status = byId(root, "profile-entity-edit-status");
    if (!toggle || !block || !form) return;

    var editPanel = byId(root, "profile-entity-edit-panel");
    var deleteBlock = byId(root, "profile-delete-entity-block");
    var deleteMsg = byId(root, "profile-delete-entity-msg");
    var entityHandle = String(block.getAttribute("data-entity-handle") || "").trim();
    var entityTypeKey = String(block.getAttribute("data-entity-type-key") || "").trim();

    bindEntityEditDescription(root);
    bindEntityEditOwnerHandles(root);
    if (
      entityTypeKey === "profit" ||
      entityTypeKey === "non_profit" ||
      entityTypeKey === "gathering"
    ) {
      bindEntityEditPlaceSearch(root, deps);
    }

    function clearStatus(el) {
      if (!el) return;
      el.hidden = true;
      el.textContent = "";
      el.classList.remove(
        "create-post-submit-status--error",
        "create-post-submit-status--success",
        "handle-availability--taken",
        "handle-availability--ready"
      );
    }
    function showStatus(el, text, isError) {
      if (!el) return;
      var t = String(text || "").trim();
      el.hidden = !t;
      el.textContent = t;
      el.classList.toggle("create-post-submit-status--error", !!isError && !!t);
      el.classList.toggle("create-post-submit-status--success", !isError && !!t);
      el.classList.toggle("handle-availability--taken", !!isError && !!t);
      el.classList.toggle("handle-availability--ready", !isError && !!t);
    }
    function clearDeleteMsg(el) {
      if (!el) return;
      el.hidden = true;
      el.textContent = "";
    }
    function showDeleteMsg(el, text) {
      if (!el) return;
      el.hidden = !text;
      el.textContent = text || "";
    }
    function setEntityDetailSectionsHidden(hidden) {
      var detailSections = byId(root, "profile-entity-detail-sections");
      if (detailSections) detailSections.hidden = hidden;
    }
    function resetPanels() {
      if (editPanel) editPanel.hidden = false;
      if (deleteBlock) deleteBlock.hidden = true;
      clearStatus(status);
      clearDeleteMsg(deleteMsg);
    }
    function setEditOpen(open) {
      block.hidden = !open;
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
      setEntityDetailSectionsHidden(open);
      if (!open) resetPanels();
      else {
        clearStatus(status);
        if (typeof deps.scrollFeedToTop === "function") deps.scrollFeedToTop();
        if (form && form.scrollIntoView) {
          window.requestAnimationFrame(function () {
            form.scrollIntoView({ block: "start", behavior: "smooth" });
          });
        }
      }
    }

    toggle.addEventListener("click", function () {
      if (block.hidden) resetPanels();
      setEditOpen(block.hidden);
    });
    if (cancelBtn) cancelBtn.addEventListener("click", function () { setEditOpen(false); });

    form.addEventListener("submit", async function (e) {
      e.preventDefault();
      if (!entityHandle) return;
      var saveBtn = byId(root, "profile-entity-edit-save");
      if (saveBtn) saveBtn.disabled = true;
      clearStatus(status);
      try {
        var descInput = byId(root, "profile-entity-edit-description");
        var descEditor = byId(root, "profile-entity-edit-description-editor");
        if (descInput && descEditor && EntityFormCommon.syncRichDescription) {
          EntityFormCommon.syncRichDescription(descInput, descEditor);
        } else if (descInput && descEditor) {
          descInput.value = String(descEditor.innerHTML || "").trim();
        }
        var fd = new FormData(form);
        var ownerRaw = EntityFormCommon.stripOwnerHandlesPrefix
          ? EntityFormCommon.stripOwnerHandlesPrefix(fd.get("owner_handles") || "")
          : (fd.get("owner_handles") || "").toString().trim();
        var draft = {
          label: (fd.get("label") || "").toString().trim(),
          legal_name: (fd.get("legal_name") || "").toString().trim() || null,
          founded_at: (fd.get("founded_at") || "").toString().trim() || null,
          description: descInput ? String(descInput.value || "").trim() || null : null,
          intention: (fd.get("intention") || "").toString().trim() || null,
          website_url: (fd.get("website_url") || "").toString().trim() || null,
          logo_url: (fd.get("logo_url") || "").toString().trim() || null,
          event_url: (fd.get("event_url") || "").toString().trim() || null,
          location_id: (fd.get("location_id") || "").toString().trim() || null,
          owner_handles: ownerRaw || null,
        };
        var r = await fetch("/me/entities/" + encodeURIComponent(entityHandle), {
          method: "PATCH",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/json" }),
          body: JSON.stringify(draft),
        });
        var payload = await r.json().catch(function () { return {}; });
        if (!r.ok) {
          throw new Error(
            (payload && payload.error) ||
              deps.UI.profile_edit_failed ||
              "Could not update profile right now"
          );
        }
        deps.notify(deps.UI.profile_edit_success || "Profile updated.", 3200);
        setEditOpen(false);
        if (deps.currentProfileHandle) {
          deps.openPrivateProfileCard(deps.currentProfileHandle, false, true);
          deps.scrollFeedToTop();
        }
      } catch (err) {
        showStatus(
          status,
          (err && err.message) || deps.UI.profile_edit_failed || "Could not update profile right now",
          true
        );
        deps.notify(
          (err && err.message) || deps.UI.profile_edit_failed || "Could not update profile right now",
          4000
        );
      } finally {
        if (saveBtn) saveBtn.disabled = false;
      }
    });

    var deleteOpen = byId(root, "profile-delete-entity-open");
    if (deleteOpen) {
      deleteOpen.addEventListener("click", function () {
        clearDeleteMsg(deleteMsg);
        if (editPanel) editPanel.hidden = true;
        if (deleteBlock) deleteBlock.hidden = false;
        deps.scrollFeedToTop();
      });
    }
    var deleteCancel = byId(root, "profile-delete-entity-cancel");
    if (deleteCancel) {
      deleteCancel.addEventListener("click", function () {
        resetPanels();
      });
    }
    var deleteSubmit = byId(root, "profile-delete-entity-submit");
    if (deleteSubmit) {
      deleteSubmit.addEventListener("click", async function () {
        if (!entityHandle) return;
        clearDeleteMsg(deleteMsg);
        var passwordEl = byId(root, "profile-delete-entity-password");
        var confirmEl = byId(root, "profile-delete-entity-password-confirm");
        var ackEl = byId(root, "profile-delete-entity-ack");
        var password = passwordEl ? String(passwordEl.value || "") : "";
        var confirm = confirmEl ? String(confirmEl.value || "") : "";
        if (!password || !confirm) {
          showDeleteMsg(
            deleteMsg,
            deps.UI.account_password_required || "Enter your password to continue."
          );
          return;
        }
        if (password !== confirm) {
          showDeleteMsg(deleteMsg, deps.UI.change_password_mismatch || "Passwords do not match.");
          return;
        }
        if (!ackEl || !ackEl.checked) {
          showDeleteMsg(
            deleteMsg,
            deps.UI.delete_entity_confirm_required ||
              deps.UI.delete_account_confirm_required ||
              "Confirm that you understand this is permanent."
          );
          return;
        }
        deleteSubmit.disabled = true;
        try {
          var r = await fetch("/me/entities/" + encodeURIComponent(entityHandle) + "/delete", {
            method: "POST",
            credentials: "same-origin",
            headers: mutationHeaders({ "Content-Type": "application/json" }),
            body: JSON.stringify({ password: password, confirm: true }),
          });
          var payload = await r.json().catch(function () { return {}; });
          if (!r.ok) {
            throw new Error(
              (payload && payload.error) ||
                deps.UI.delete_entity_failed ||
                deps.UI.delete_account_failed ||
                "Could not delete this profile right now."
            );
          }
          deps.notify(
            deps.UI.delete_entity_success || deps.UI.delete_account_success || "Profile deleted.",
            3200
          );
          if (global.ProfileCard && typeof global.ProfileCard.dismiss === "function") {
            global.ProfileCard.dismiss(deps);
          }
          if (payload && payload.redirect) {
            global.location.href = payload.redirect;
          }
        } catch (err) {
          showDeleteMsg(
            deleteMsg,
            (err && err.message) ||
              deps.UI.delete_entity_failed ||
              deps.UI.delete_account_failed ||
              "Could not delete this profile right now."
          );
        } finally {
          deleteSubmit.disabled = false;
        }
      });
    }

    root.setAttribute("data-profile-entity-edit-bound", "1");
  }

  function bindAvatarRedraw(root, deps) {
    if (!root) return;
    var btn = root.querySelector("#profile-avatar-redraw-btn");
    if (!btn || btn.getAttribute("data-avatar-redraw-bound") === "1") return;
    btn.setAttribute("data-avatar-redraw-bound", "1");
    btn.addEventListener("click", function () {
      if (typeof deps.openProfileAvatarRedrawCapture === "function") {
        deps.openProfileAvatarRedrawCapture();
      }
    });
  }

    function bindRecruitmentInviteBulk(root, deps) {
    var block = root ? root.querySelector("#profile-recruitment-invite-block") : null;
    if (!block || block.getAttribute("data-recruitment-bound") === "1") return;
    block.setAttribute("data-recruitment-bound", "1");
    var textarea = byId(root, "profile-recruitment-invite-emails");
    var messageField = byId(root, "profile-recruitment-invite-message");
    var sendBtn = byId(root, "profile-recruitment-invite-send");
    var status = byId(root, "profile-recruitment-invite-status");
    var handle = block.getAttribute("data-entity-handle") || "";
    var UI = (deps && deps.UI) || {};

    function showStatus(msg, isError) {
      if (!status) return;
      status.hidden = !msg;
      status.textContent = msg || "";
      status.classList.toggle("create-post-submit-status--error", !!isError);
    }

    function hasValidRecipient(text) {
      var parts = String(text || "").split(/[\s,;]+/);
      for (var i = 0; i < parts.length; i++) {
        var t = parts[i].trim();
        if (t.length >= 3 && t.length <= 254 && t.indexOf("@") !== -1) return true;
      }
      return false;
    }

    function syncSendEnabled() {
      if (!sendBtn || !textarea) return;
      sendBtn.disabled = !hasValidRecipient(textarea.value);
    }

    if (textarea) {
      textarea.addEventListener("input", syncSendEnabled);
      syncSendEnabled();
    }

    if (!sendBtn || !handle) return;
    sendBtn.addEventListener("click", function () {
      if (!textarea) return;
      var text = String(textarea.value || "").trim();
      var personalMessage = messageField ? String(messageField.value || "").trim() : "";
      if (!hasValidRecipient(text)) {
        showStatus(UI.friend_invite_invalid_email || "Enter an email address to invite.", true);
        return;
      }
      sendBtn.disabled = true;
      showStatus("", false);
      fetch(
        "/me/entities/" + encodeURIComponent(handle) + "/recruitment-invites",
        {
          method: "POST",
          credentials: "same-origin",
          headers: mutationHeaders({ "Content-Type": "application/json" }),
          body: JSON.stringify({ emails: [], emails_text: text, personal_message: personalMessage }),
        }
      )
        .then(function (r) {
          return r.json().catch(function () { return {}; }).then(function (payload) {
            if (!r.ok) {
              var err = new Error(
                (payload && payload.error) ||
                  UI.friend_invite_failed ||
                  "Could not send invitations right now."
              );
              err.rateMsg = err.message;
              throw err;
            }
            return payload;
          });
        })
        .then(function (payload) {
          textarea.value = "";
          if (messageField) messageField.value = "";
          showStatus(
            (payload && payload.message) ||
              UI.recruitment_invite_bulk_result ||
              UI.friend_invite_sent ||
              "Invitations sent.",
            false
          );
          syncSendEnabled();
        })
        .catch(function (err) {
          showStatus(
            (err && err.rateMsg) ||
              UI.friend_invite_failed ||
              "Could not send invitations right now.",
            true
          );
          syncSendEnabled();
        });
    });
  }

  function bindCommunityInviteSearch(root, deps) {
    var block = root ? root.querySelector("#profile-community-invite-block") : null;
    if (!block || block.getAttribute("data-community-invite-bound") === "1") return;
    block.setAttribute("data-community-invite-bound", "1");
    if (typeof deps.bindCommunityInviteSearch === "function") {
      deps.bindCommunityInviteSearch(root);
    }
  }

  function bindAll(root, deps) {
    if (!root) return;
    var legacyLogout = root.querySelector("#profile-logout-block, .profile-logout-block");
    if (legacyLogout) legacyLogout.remove();
    bindConnectForm(root, deps);
    bindInviteResponseForm(root, deps);
    bindSimplePostForm(root, "profile-cancel-invite-form", deps, function () {
      showConnectNotice(root, deps.UI.invitation_cancelled_notice || "", deps);
    });
    bindSimplePostForm(root, "profile-disconnect-form", deps, function () {
      showConnectNotice(root, deps.UI.disconnected_notice || "", deps);
    });
    bindBlockForm(root, deps);
    bindAvatarRedraw(root, deps);
    bindHumanEditForm(root, deps);
    bindEntityEditForm(root, deps);
    bindSimplePostForm(root, "profile-leave-community-form", deps, function () {
      var lb = byId(root, "profile-leave-community-block");
      if (lb) {
        lb.innerHTML =
          '<p class="profile-value">' + deps.escapeHtml(deps.UI.community_left_notice || "") + "</p>";
      }
    });
    bindRecruitmentInviteBulk(root, deps);
    bindCommunityInviteSearch(root, deps);
    if (typeof deps.bindCommunityInviteSendForm === "function") deps.bindCommunityInviteSendForm(root);
    if (typeof deps.bindCommunityInviteResponseForm === "function") {
      deps.bindCommunityInviteResponseForm(root);
    }
    bindSimplePostForm(root, "profile-follow-form", deps, function () {
      var fb = byId(root, "profile-entity-follow-block");
      if (fb) {
        fb.innerHTML =
          '<p class="profile-value">' + deps.escapeHtml(deps.UI.following_saved_notice || "") + "</p>";
      }
    });
    bindSimplePostForm(root, "profile-unfollow-form", deps, function () {
      var fb = byId(root, "profile-entity-follow-block");
      if (fb) {
        fb.innerHTML =
          '<p class="profile-value">' + deps.escapeHtml(deps.UI.unfollowed_notice || "") + "</p>";
      }
    });
    bindGatheringRsvpForm(root, deps);
    bindAboutExpander(root, deps.UI);
    bindProfileViewToggle(root, deps);
    var detailSections = byId(root, "profile-entity-detail-sections");
    if (!detailSections || !detailSections.hidden) {
      bindConstitutionExpander(root, deps.UI);
    }
  }

  function mount(deps, html, handle, viewNonce, pushUrl) {
    deps.renderFeedShell();
    var existing = document.getElementById("app-private-profile-card-slot");
    if (existing) existing.remove();
    var slot = document.createElement("div");
    slot.id = "app-private-profile-card-slot";
    slot.className = "app-private-profile-card-slot";
    slot.innerHTML = html;
    deps.feedPanel.insertBefore(slot, deps.feedPanel.firstChild);
    bindAll(slot, deps);
    deps.bindFeedControls();
    deps.loadFeedDay(0, viewNonce);
    deps.ensureRightPanelNotificationHost();
    if (typeof deps.applyPrivateProfileTitle === "function") deps.applyPrivateProfileTitle(handle);
    deps.scrollFeedToTop();
    if (pushUrl && global.history && global.history.pushState) {
      global.history.pushState(
        { panel: "private-profile", handle: handle },
        "",
        "/home/profile/" + encodeURIComponent(handle)
      );
    }
  }

  function initEmbeddedSlot(deps, slot, handle, viewNonce, pushUrl) {
    if (!slot) return;
    slot.id = "app-private-profile-card-slot";
    slot.className = "app-private-profile-card-slot";
    bindAll(slot, deps);
    deps.bindFeedControls();
    deps.loadFeedDay(0, viewNonce);
    deps.ensureRightPanelNotificationHost();
    if (typeof deps.applyPrivateProfileTitle === "function") deps.applyPrivateProfileTitle(handle);
    deps.scrollFeedToTop();
    if (pushUrl && global.history && global.history.pushState) {
      global.history.pushState(
        { panel: "private-profile", handle: handle },
        "",
        "/home/profile/" + encodeURIComponent(handle)
      );
    }
  }

  global.ProfileCard = {
    csrfToken: csrfToken,
    mutationHeaders: mutationHeaders,
    dismiss: dismissProfileCard,
    mount: mount,
    initEmbeddedSlot: initEmbeddedSlot,
    bind: bindAll,
  };
})(window);

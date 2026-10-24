/**
 * Profile edit account security: change password and delete account panels.
 */
(function () {
  "use strict";

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

  function passwordMinLen(block, UI) {
    var n = Number(block.getAttribute("data-password-min-len") || UI.password_min_len || 12);
    return Number.isFinite(n) && n > 0 ? n : 12;
  }

  function mutationHeaders(extra) {
    if (typeof opts.mutationHeaders === "function") return opts.mutationHeaders(extra);
    if (window.ProfileCard && typeof window.ProfileCard.mutationHeaders === "function") {
      return window.ProfileCard.mutationHeaders(extra);
    }
    var h = { Accept: "application/json" };
    if (extra) {
      for (var k in extra) {
        if (Object.prototype.hasOwnProperty.call(extra, k)) h[k] = extra[k];
      }
    }
    return h;
  }

  function bindProfileAccountPanels(opts) {
    var root = opts.root || null;
    function byId(id) {
      if (root) {
        return root.querySelector("#" + id);
      }
      return document.getElementById(id);
    }

    var block = opts.block;
    var UI = opts.UI || {};
    var editPanel = opts.editPanel;
    var passwordBlock = opts.passwordBlock;
    var passwordVerifyStep = opts.passwordVerifyStep;
    var passwordNewStep = opts.passwordNewStep;
    var passwordMsg = opts.passwordMsg;
    var deleteBlock = opts.deleteBlock;
    var blocksBlock = opts.blocksBlock;
    var blocksList = opts.blocksList;
    var blocksEmpty = opts.blocksEmpty;
    var blocksMsg = opts.blocksMsg;
    var deleteMsg = opts.deleteMsg;
    var scrollFeedToTop = opts.scrollFeedToTop || function () {};
    var notify = opts.notify || function () {};

    if (!block) return;
    if (root && root.getAttribute("data-profile-account-bound") === "1") return;
    if (!root && block.getAttribute("data-profile-account-bound") === "1") return;
    if (root) root.setAttribute("data-profile-account-bound", "1");
    else block.setAttribute("data-profile-account-bound", "1");

    var skipVerifyStep =
      passwordBlock &&
      passwordBlock.getAttribute("data-skip-verify") === "1";

    function resetPasswordFields() {
      [
        "profile-current-password",
        "profile-current-password-confirm",
        "profile-new-password",
        "profile-new-password-confirm",
      ].forEach(function (id) {
        var el = byId(id);
        if (el) el.value = "";
      });
      clearMsg(passwordMsg);
      if (passwordVerifyStep) passwordVerifyStep.hidden = skipVerifyStep;
      if (passwordNewStep) passwordNewStep.hidden = !skipVerifyStep;
    }

    function resetDeleteFields() {
      ["profile-delete-password", "profile-delete-password-confirm"].forEach(function (id) {
        var el = byId(id);
        if (el) el.value = "";
      });
      var ack = byId("profile-delete-account-ack");
      if (ack) ack.checked = false;
      clearMsg(deleteMsg);
    }

    function showEditSubview(view) {
      if (editPanel) editPanel.hidden = view !== "profile";
      if (passwordBlock) passwordBlock.hidden = view !== "password";
      if (blocksBlock) blocksBlock.hidden = view !== "blocks";
      if (deleteBlock) deleteBlock.hidden = view !== "delete";
    }

    function renderBlockList(entries) {
      if (!blocksList || !blocksEmpty) return;
      blocksList.innerHTML = "";
      var rows = Array.isArray(entries) ? entries : [];
      if (!rows.length) {
        blocksList.hidden = true;
        blocksEmpty.hidden = false;
        return;
      }
      blocksEmpty.hidden = true;
      blocksList.hidden = false;
      rows.forEach(function (row) {
        var handle = String((row && row.handle) || "").trim();
        if (!handle) return;
        var li = document.createElement("li");
        li.className = "profile-block-list-item";
        var label = document.createElement("span");
        label.className = "profile-block-list-label";
        label.textContent = "@" + handle;
        var btn = document.createElement("button");
        btn.type = "button";
        btn.className = "auth-text-btn btn-action profile-connect-cta-btn profile-block-unblock-btn";
        btn.setAttribute("data-block-handle", handle);
        btn.textContent = UI.unblock_cta || "Unblock";
        li.appendChild(label);
        li.appendChild(btn);
        blocksList.appendChild(li);
      });
    }

    async function loadBlockList() {
      clearMsg(blocksMsg);
      if (blocksList) {
        blocksList.hidden = true;
        blocksList.innerHTML = "";
      }
      if (blocksEmpty) blocksEmpty.hidden = true;
      try {
        var r = await fetch("/me/block-list", {
          credentials: "same-origin",
          headers: { Accept: "application/json" },
        });
        var payload = await r.json().catch(function () {
          return [];
        });
        if (!r.ok) {
          throw new Error(
            (payload && payload.error) ||
              UI.manage_blocks_load_failed ||
              "Could not load your block list right now."
          );
        }
        renderBlockList(payload);
      } catch (err) {
        showMsg(
          blocksMsg,
          (err && err.message) ||
            UI.manage_blocks_load_failed ||
            "Could not load your block list right now."
        );
      }
    }

    function resetAccountPanels() {
      showEditSubview("profile");
      resetPasswordFields();
      resetDeleteFields();
    }

    var changePasswordOpen = byId("profile-change-password-open");
    if (changePasswordOpen) {
      changePasswordOpen.addEventListener("click", function () {
        resetPasswordFields();
        showEditSubview("password");
        scrollFeedToTop();
      });
    }
    var changePasswordCancel = byId("profile-change-password-cancel");
    if (changePasswordCancel) {
      changePasswordCancel.addEventListener("click", function () {
        resetPasswordFields();
        showEditSubview("profile");
      });
    }
    var changePasswordBack = byId("profile-change-password-back");
    if (changePasswordBack && !skipVerifyStep) {
      changePasswordBack.addEventListener("click", function () {
        clearMsg(passwordMsg);
        if (passwordVerifyStep) passwordVerifyStep.hidden = false;
        if (passwordNewStep) passwordNewStep.hidden = true;
      });
    } else if (changePasswordBack) {
      changePasswordBack.hidden = true;
    }
    var changePasswordContinue = byId("profile-change-password-continue");
    if (changePasswordContinue) {
      changePasswordContinue.addEventListener("click", function () {
        clearMsg(passwordMsg);
        var currentEl = byId("profile-current-password");
        var confirmEl = byId("profile-current-password-confirm");
        var current = currentEl ? String(currentEl.value || "") : "";
        var confirm = confirmEl ? String(confirmEl.value || "") : "";
        if (!current || !confirm) {
          showMsg(
            passwordMsg,
            UI.account_password_required || "Enter your password to continue."
          );
          return;
        }
        if (current !== confirm) {
          showMsg(passwordMsg, UI.change_password_mismatch || "Passwords do not match.");
          return;
        }
        if (passwordVerifyStep) passwordVerifyStep.hidden = true;
        if (passwordNewStep) passwordNewStep.hidden = false;
      });
    }
    var changePasswordSave = byId("profile-change-password-save");
    if (changePasswordSave) {
      changePasswordSave.addEventListener("click", async function () {
        clearMsg(passwordMsg);
        var currentEl = byId("profile-current-password");
        var newEl = byId("profile-new-password");
        var confirmEl = byId("profile-new-password-confirm");
        var current = currentEl ? String(currentEl.value || "") : "";
        var newPassword = newEl ? String(newEl.value || "") : "";
        var confirm = confirmEl ? String(confirmEl.value || "") : "";
        var minLen = passwordMinLen(block, UI);
        if (!newPassword || !confirm) {
          showMsg(
            passwordMsg,
            UI.account_password_required || "Enter your password to continue."
          );
          return;
        }
        if (newPassword !== confirm) {
          showMsg(passwordMsg, UI.change_password_mismatch || "Passwords do not match.");
          return;
        }
        if (newPassword.length < minLen) {
          showMsg(
            passwordMsg,
            UI.password_too_short || "Password must be at least " + minLen + " characters."
          );
          return;
        }
        if (newPassword === current && !skipVerifyStep) {
          showMsg(
            passwordMsg,
            UI.account_password_same_as_current ||
              "Choose a password different from your current one."
          );
          return;
        }
        changePasswordSave.disabled = true;
        try {
          var r = await fetch("/me/change-password", {
            method: "POST",
            credentials: "same-origin",
            headers: mutationHeaders({ "Content-Type": "application/json" }),
            body: JSON.stringify({
              current_password: skipVerifyStep ? "" : current,
              new_password: newPassword,
            }),
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            throw new Error(
              (payload && payload.error) ||
                UI.change_password_failed ||
                "Could not change your password right now"
            );
          }
          if (skipVerifyStep && payload && payload.redirect) {
            window.location.href = payload.redirect;
            return;
          }
          notify(UI.change_password_success || "Password updated.", 3200);
          resetPasswordFields();
          showEditSubview("profile");
        } catch (err) {
          showMsg(
            passwordMsg,
            (err && err.message) ||
              UI.change_password_failed ||
              "Could not change your password right now"
          );
        } finally {
          changePasswordSave.disabled = false;
        }
      });
    }

    var deleteOpen = byId("profile-delete-account-open");
    if (deleteOpen) {
      deleteOpen.addEventListener("click", function () {
        resetDeleteFields();
        showEditSubview("delete");
        scrollFeedToTop();
      });
    }
    var blocksOpen = byId("profile-manage-blocks-open");
    if (blocksOpen) {
      blocksOpen.addEventListener("click", function () {
        clearMsg(blocksMsg);
        showEditSubview("blocks");
        scrollFeedToTop();
        loadBlockList();
      });
    }
    var blocksCancel = byId("profile-manage-blocks-cancel");
    if (blocksCancel) {
      blocksCancel.addEventListener("click", function () {
        clearMsg(blocksMsg);
        showEditSubview("profile");
      });
    }
    if (blocksList) {
      blocksList.addEventListener("click", async function (e) {
        var target = e.target;
        if (!target || !target.classList || !target.classList.contains("profile-block-unblock-btn")) {
          return;
        }
        var handle = String(target.getAttribute("data-block-handle") || "").trim();
        if (!handle) return;
        clearMsg(blocksMsg);
        target.disabled = true;
        try {
          var r = await fetch("/me/blocks/unblock", {
            method: "POST",
            credentials: "same-origin",
            headers: mutationHeaders({ "Content-Type": "application/json" }),
            body: JSON.stringify({ handle: handle }),
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            throw new Error(
              (payload && payload.error) ||
                UI.unblock_failed ||
                "Could not unblock that person right now."
            );
          }
          notify(
            (payload && payload.message) || UI.unblock_success || "Unblocked.",
            3200
          );
          await loadBlockList();
        } catch (err) {
          showMsg(
            blocksMsg,
            (err && err.message) ||
              UI.unblock_failed ||
              "Could not unblock that person right now."
          );
          target.disabled = false;
        }
      });
    }
    var deleteCancel = byId("profile-delete-account-cancel");
    if (deleteCancel) {
      deleteCancel.addEventListener("click", function () {
        resetDeleteFields();
        showEditSubview("profile");
      });
    }
    var deleteSubmit = byId("profile-delete-account-submit");
    if (deleteSubmit) {
      deleteSubmit.addEventListener("click", async function () {
        clearMsg(deleteMsg);
        var passwordEl = byId("profile-delete-password");
        var confirmEl = byId("profile-delete-password-confirm");
        var ackEl = byId("profile-delete-account-ack");
        var password = passwordEl ? String(passwordEl.value || "") : "";
        var confirm = confirmEl ? String(confirmEl.value || "") : "";
        if (!password || !confirm) {
          showMsg(
            deleteMsg,
            UI.account_password_required || "Enter your password to continue."
          );
          return;
        }
        if (password !== confirm) {
          showMsg(deleteMsg, UI.change_password_mismatch || "Passwords do not match.");
          return;
        }
        if (!ackEl || !ackEl.checked) {
          showMsg(
            deleteMsg,
            UI.delete_account_confirm_required ||
              "Confirm that you understand this is permanent."
          );
          return;
        }
        deleteSubmit.disabled = true;
        try {
          var r = await fetch("/me/account/delete", {
            method: "POST",
            credentials: "same-origin",
            headers: mutationHeaders({ "Content-Type": "application/json" }),
            body: JSON.stringify({ password: password, confirm: true }),
          });
          var payload = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            throw new Error(
              (payload && payload.error) ||
                UI.delete_account_failed ||
                "Could not delete your account right now"
            );
          }
          window.location.href = (payload && payload.redirect) || "/login";
        } catch (err) {
          showMsg(
            deleteMsg,
            (err && err.message) ||
              UI.delete_account_failed ||
              "Could not delete your account right now"
          );
          deleteSubmit.disabled = false;
        }
      });
    }

    return {
      resetAccountPanels: resetAccountPanels,
      showEditSubview: showEditSubview,
    };
  }

  window.ProfileAccount = {
    bind: bindProfileAccountPanels,
  };
})();

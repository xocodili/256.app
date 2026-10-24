/**
 * Nom pack purchase — Stripe Payment Element (card only) from topbar wallet button.
 */
(function () {
  "use strict";

  var STRIPE_JS = "https://js.stripe.com/v3/";
  var pollMs = 800;
  var pollMax = 40;

  function csrfHeaders() {
    var h = { Accept: "application/json", "Content-Type": "application/json" };
    var meta = document.querySelector('meta[name="csrf-token"]');
    var token = meta && meta.getAttribute("content");
    if (token) h["X-CSRF-Token"] = token;
    return h;
  }

  function loadStripeJs() {
    return new Promise(function (resolve, reject) {
      if (window.Stripe) {
        resolve();
        return;
      }
      var existing = document.querySelector('script[src="' + STRIPE_JS + '"]');
      if (existing) {
        existing.addEventListener("load", function () { resolve(); });
        existing.addEventListener("error", reject);
        return;
      }
      var s = document.createElement("script");
      s.src = STRIPE_JS;
      s.async = true;
      s.onload = function () { resolve(); };
      s.onerror = reject;
      document.head.appendChild(s);
    });
  }

  function i18n(key, fallback) {
    var el = document.getElementById("nom-purchase-i18n");
    if (el && el.textContent) {
      try {
        var pack = JSON.parse(el.textContent);
        if (pack && pack[key]) return String(pack[key]);
      } catch (_e) { /* ignore */ }
    }
    var UI = window.__HOME_SHELL_I18N__;
    if (UI && UI[key]) return String(UI[key]);
    return fallback;
  }

  function updateTopbarBalance(balance) {
    var nomsEl = document.querySelector(".app-topbar-ai-tokens .nom-amount-count");
    if (!nomsEl) {
      nomsEl = document.querySelector(".app-topbar-ai-tokens-count");
    }
    if (nomsEl) nomsEl.textContent = String(balance);
    var nomsWrap = document.getElementById("app-topbar-noms-btn") || document.querySelector(".app-topbar-ai-tokens");
    if (nomsWrap) {
      var tpl = i18n("noms_wallet_aria", "Noms: {balance}");
      nomsWrap.setAttribute("aria-label", tpl.replace("{balance}", String(balance)));
    }
  }

  function ensureModal() {
    var el = document.getElementById("nom-purchase-modal");
    if (el) return el;
    el = document.createElement("div");
    el.id = "nom-purchase-modal";
    el.className = "nom-purchase-modal";
    el.setAttribute("role", "dialog");
    el.setAttribute("aria-modal", "true");
    el.hidden = true;
    el.innerHTML =
      '<div class="nom-purchase-modal__backdrop" data-nom-purchase-close="1"></div>' +
      '<div class="nom-purchase-modal__panel">' +
      '  <div class="card auth-card app-card nom-purchase-card">' +
      '    <div class="card-accent nom-purchase-card__accent" role="presentation"></div>' +
      '    <h2 class="nom-purchase-card__title" id="nom-purchase-title"></h2>' +
      '    <fieldset class="nom-purchase-pack-picker" id="nom-purchase-pack-picker">' +
      '      <legend class="nom-purchase-pack-picker__legend" id="nom-purchase-pack-legend"></legend>' +
      '      <div class="nom-purchase-pack-options" id="nom-purchase-pack-options"></div>' +
      '    </fieldset>' +
      '    <p class="sub nom-purchase-card__hint" id="nom-purchase-hint"></p>' +
      '    <p class="sub nom-purchase-card__err" id="nom-purchase-err" hidden role="alert"></p>' +
      '    <div id="nom-purchase-processing" class="nom-purchase-processing" hidden role="status" aria-live="polite" aria-busy="true">' +
      '      <div id="nom-purchase-processing-loader"></div>' +
      '      <p class="nom-purchase-processing__msg" id="nom-purchase-processing-msg"></p>' +
      '    </div>' +
      '    <div id="nom-purchase-payment-wrap" class="nom-purchase-payment-wrap" hidden>' +
      '      <div id="nom-purchase-payment" class="nom-purchase-payment"></div>' +
      '      <p class="nom-purchase-stripe-note" id="nom-purchase-stripe-note"></p>' +
      '    </div>' +
      '    <button type="button" class="auth-text-btn btn-cta nom-purchase-pay" id="nom-purchase-pay" hidden disabled></button>' +
      '    <button type="button" class="auth-text-btn nom-purchase-cancel" data-nom-purchase-close="1"></button>' +
      "  </div>" +
      "</div>";
    document.body.appendChild(el);
    el.querySelectorAll("[data-nom-purchase-close]").forEach(function (node) {
      node.addEventListener("click", function () {
        if (payInFlight) return;
        closeModal();
      });
    });
    return el;
  }

  var activeStripe = null;
  var activeElements = null;
  var activePayment = null;
  var activeConfig = null;
  var selectedSku = null;
  var activeClientSecret = null;

  var activePurchaseId = null;
  var checkoutInFlight = false;
  var payInFlight = false;
  var cardComplete = false;

  function updatePayButtonState() {
    var payBtn = document.getElementById("nom-purchase-pay");
    if (!payBtn) return;
    var show =
      !!activeClientSecret &&
      !!activePayment &&
      cardComplete &&
      !payInFlight &&
      !checkoutInFlight;
    payBtn.hidden = !show;
    payBtn.disabled = !show;
  }

  function packDisabledNote(pack) {
    if (pack.can_purchase) return "";
    if (pack.block_reason === "wallet_cap") {
      return " — " + i18n("noms_purchase_wallet_cap", "Would exceed the 256 Nom wallet cap.");
    }
    if (pack.block_reason === "steward_tier" || pack.wallet_full) {
      return (
        " — " +
        i18n(
          "noms_purchase_pack_requires_below",
          "Requires balance below {max}"
        ).replace("{max}", String(pack.max_balance_exclusive != null ? pack.max_balance_exclusive : pack.noms))
      );
    }
    return "";
  }

  function cancelPendingPurchase(purchaseId) {
    if (!purchaseId) return Promise.resolve();
    return fetch("/api/noms/purchase/" + encodeURIComponent(String(purchaseId)) + "/cancel", {
      method: "POST",
      headers: csrfHeaders(),
      credentials: "same-origin",
    }).catch(function () {
      /* best-effort — server may already have settled */
    });
  }

  function showPurchaseNotify(msg, visibleMs) {
    if (typeof window.showHomeToast === "function") {
      window.showHomeToast(msg, visibleMs || 3200);
      return;
    }
    if (msg) alert(msg);
  }

  function ensureProcessingLoader() {
    var loaderHost = document.getElementById("nom-purchase-processing-loader");
    if (!loaderHost || loaderHost.childElementCount) return;
    if (window.brandCardLoader && typeof window.brandCardLoader.cardLoaderHtml === "function") {
      loaderHost.innerHTML = window.brandCardLoader.cardLoaderHtml();
    }
  }

  function setPackPickerInteractive(enabled) {
    var picker = document.getElementById("nom-purchase-pack-picker");
    if (!picker) return;
    picker.classList.toggle("is-processing", !enabled);
    picker.querySelectorAll("input[type=\"radio\"]").forEach(function (input) {
      if (enabled) {
        input.disabled = !!input.closest(".nom-purchase-pack-option--disabled");
      } else {
        input.disabled = true;
      }
    });
  }

  function setCheckoutActionsVisible(visible) {
    var cancelBtn = document.querySelector(".nom-purchase-cancel");
    if (cancelBtn) cancelBtn.hidden = !visible;
    if (visible) {
      setPaymentWrapVisible(true);
      updatePayButtonState();
    } else {
      setPaymentWrapVisible(false);
    }
  }

  function setProcessingMessage(stageKey, fallback) {
    var msgEl = document.getElementById("nom-purchase-processing-msg");
    if (!msgEl) return;
    msgEl.textContent = i18n(stageKey, fallback);
  }

  function setProcessingState(isProcessing, stageKey, stageFallback) {
    var processingEl = document.getElementById("nom-purchase-processing");
    if (isProcessing) {
      ensureProcessingLoader();
      setProcessingMessage(
        stageKey || "noms_purchase_processing",
        stageFallback || "Thank you, we're processing that transaction."
      );
      if (processingEl) processingEl.hidden = false;
      showError("");
      setCheckoutActionsVisible(false);
      setPackPickerInteractive(false);
      return;
    }
    if (processingEl) processingEl.hidden = true;
    setPackPickerInteractive(true);
    if (activeClientSecret) setCheckoutActionsVisible(true);
  }

  function setPayVisible(visible) {
    if (!visible) {
      var payBtn = document.getElementById("nom-purchase-pay");
      if (payBtn) {
        payBtn.hidden = true;
        payBtn.disabled = true;
      }
      return;
    }
    updatePayButtonState();
  }

  function setPaymentWrapVisible(visible) {
    var wrap = document.getElementById("nom-purchase-payment-wrap");
    if (wrap) wrap.hidden = !visible;
    if (!visible) setPayVisible(false);
  }

  function updateStripeNote() {
    var note = document.getElementById("nom-purchase-stripe-note");
    if (!note) return;
    var label = i18n(
      "noms_purchase_stripe_powered",
      "Payments processed securely by Stripe."
    );
    note.innerHTML =
      '<a href="https://stripe.com" target="_blank" rel="noopener noreferrer">' +
      label +
      "</a>";
  }

  function billingCountry(config) {
    var cc = config && config.billing_country;
    if (cc && String(cc).trim().length === 2) {
      return String(cc).trim().toUpperCase();
    }
    return "AU";
  }

  function paymentElementOptions(config) {
    var country = billingCountry(config);
    return {
      wallets: {
        applePay: "never",
        googlePay: "never",
        link: "never",
      },
      fields: {
        billingDetails: {
          // `never` requires every omitted field at confirmPayment time (incl. postal_code).
          // `if_required` lets Stripe collect postal code when the card network needs it.
          address: "if_required",
        },
      },
      defaultValues: {
        billingDetails: {
          address: {
            country: country,
          },
        },
      },
    };
  }

  function confirmPaymentParams() {
    return {
      return_url: window.location.href,
    };
  }

  function stripeAppearance() {
    return {
      theme: "night",
      variables: {
        colorPrimary: "#f5d000",
        colorBackground: "#1a1d26",
        colorText: "#f0f2f7",
        colorTextSecondary: "#b8c0cc",
        colorTextPlaceholder: "#8b95a5",
        colorDanger: "#e63946",
        borderRadius: "6px",
      },
      rules: {
        ".Input": {
          backgroundColor: "#1a1d26",
          border: "1px solid rgba(255, 255, 255, 0.14)",
          boxShadow: "none",
        },
        ".Input:focus": {
          border: "1px solid #f5d000",
          boxShadow: "0 0 0 1px #f5d000",
        },
        ".Label": {
          color: "#b8c0cc",
        },
      },
    };
  }

  function teardownCheckout() {
    if (activePayment) {
      try {
        activePayment.destroy();
      } catch (_e) { /* ignore */ }
      activePayment = null;
    }
    activeElements = null;
    activeStripe = null;
    activeClientSecret = null;
    cardComplete = false;
    var mount = document.getElementById("nom-purchase-payment");
    if (mount) mount.innerHTML = "";
    setPaymentWrapVisible(false);
  }

  function closeModal() {
    if (payInFlight) return;
    var pendingId = activePurchaseId;
    activePurchaseId = null;
    var el = document.getElementById("nom-purchase-modal");
    if (el) el.hidden = true;
    teardownCheckout();
    activeConfig = null;
    selectedSku = null;
    payInFlight = false;
    setProcessingState(false);
    document.body.classList.remove("nom-purchase-modal-open");
    if (pendingId) cancelPendingPurchase(pendingId);
  }

  function showError(msg) {
    var err = document.getElementById("nom-purchase-err");
    if (!err) return;
    if (msg) {
      err.textContent = msg;
      err.hidden = false;
    } else {
      err.textContent = "";
      err.hidden = true;
    }
  }

  function packLine(pack) {
    var tpl = i18n("noms_purchase_pack_line", "{noms} Noms — {price}");
    var line = tpl
      .replace("{noms}", String(pack.noms || 0))
      .replace("{price}", pack.display_price || "");
    if (pack.label) {
      line += " (" + pack.label + ")";
    }
    return line;
  }

  function selectedPack() {
    if (!activeConfig || !activeConfig.packs || !selectedSku) return null;
    return activeConfig.packs.find(function (p) { return p.sku === selectedSku; }) || null;
  }

  function renderPackPicker(config) {
    var optionsEl = document.getElementById("nom-purchase-pack-options");
    var legend = document.getElementById("nom-purchase-pack-legend");
    var picker = document.getElementById("nom-purchase-pack-picker");
    if (!optionsEl || !legend || !picker) return;

    legend.textContent =
      i18n("noms_purchase_select_pack", "Choose a pack") +
      " · " +
      i18n("noms_purchase_balance_line", "Your balance: {balance} Noms").replace(
        "{balance}",
        String(config.balance != null ? config.balance : 0)
      );
    optionsEl.innerHTML = "";

    var packs = config.packs || [];
    if (!packs.length) {
      picker.hidden = true;
      return;
    }
    picker.hidden = false;

    packs.forEach(function (pack) {
      var id = "nom-pack-" + pack.sku;
      var label = document.createElement("label");
      label.className = "nom-purchase-pack-option";
      if (!pack.can_purchase) label.classList.add("nom-purchase-pack-option--disabled");

      var input = document.createElement("input");
      input.type = "radio";
      input.name = "nom-pack-sku";
      input.id = id;
      input.value = pack.sku;
      input.disabled = !pack.can_purchase;
      if (pack.sku === selectedSku) input.checked = true;

      input.addEventListener("change", function () {
        if (!input.checked || !pack.can_purchase) return;
        selectedSku = pack.sku;
        var hint = document.getElementById("nom-purchase-hint");
        if (hint) {
          hint.textContent = i18n(
            "noms_purchase_hint",
            "Noms are added when payment succeeds."
          );
        }
        startCheckout(config);
      });

      var text = document.createElement("span");
      text.className = "nom-purchase-pack-option__text";
      text.textContent = packLine(pack) + packDisabledNote(pack);

      label.appendChild(input);
      label.appendChild(text);
      optionsEl.appendChild(label);
    });
  }

  function anyPurchasable(packs) {
    return (packs || []).some(function (p) { return p.can_purchase; });
  }

  function openModal(config) {
    ensureModal();
    updateStripeNote();
    var el = document.getElementById("nom-purchase-modal");
    document.getElementById("nom-purchase-title").textContent = i18n(
      "noms_purchase_title",
      "Get Noms"
    );
    document.querySelector(".nom-purchase-cancel").textContent = i18n(
      "noms_purchase_cancel",
      "Cancel"
    );
    var payBtn = document.getElementById("nom-purchase-pay");
    if (payBtn) {
      payBtn.textContent = i18n("noms_purchase_pay", "Pay now");
      payBtn.hidden = true;
      payBtn.disabled = true;
    }
    var hint = document.getElementById("nom-purchase-hint");
    showError("");
    activeConfig = config;
    selectedSku = null;
    payInFlight = false;
    teardownCheckout();
    setProcessingState(false);

    if (!config.enabled) {
      document.getElementById("nom-purchase-pack-picker").hidden = true;
      hint.textContent = i18n(
        "noms_purchase_unavailable",
        "Nom purchases are not available yet."
      );
      el.hidden = false;
      document.body.classList.add("nom-purchase-modal-open");
      return;
    }

    renderPackPicker(config);

    if (!anyPurchasable(config.packs)) {
      if (config.pending_checkout) {
        hint.textContent = i18n(
          "noms_purchase_checkout_pending",
          "You already have a checkout in progress. Complete or cancel that payment before starting another."
        );
      } else {
        var blocked = (config.packs || []).filter(function (p) { return p.wallet_full; });
        var max = "128";
        if (blocked.length) {
          var thresholds = blocked.map(function (p) {
            return p.max_balance_exclusive != null ? p.max_balance_exclusive : p.noms;
          });
          max = String(Math.max.apply(null, thresholds));
        }
        hint.textContent = i18n(
          "noms_purchase_wallet_full",
          "Spend Noms until your balance is below {max} before buying this pack."
        )
          .replace("{balance}", String(config.balance || 0))
          .replace("{max}", max);
      }
      el.hidden = false;
      document.body.classList.add("nom-purchase-modal-open");
      return;
    }

    hint.textContent = i18n(
      "noms_purchase_choose_pack_hint",
      "Choose a pack to enter card details."
    );
    el.hidden = false;
    document.body.classList.add("nom-purchase-modal-open");
  }

  function pollPurchase(purchaseId) {
    var attempts = 0;
    return new Promise(function (resolve, reject) {
      function tick() {
        attempts += 1;
        fetch("/api/noms/purchase/" + encodeURIComponent(String(purchaseId)), {
          headers: { Accept: "application/json" },
          credentials: "same-origin",
        })
          .then(function (r) {
            if (!r.ok) throw new Error("status");
            return r.json();
          })
          .then(function (data) {
            if (data && data.status === "completed" && typeof data.balance_after === "number") {
              resolve(data);
              return;
            }
            if (data && data.status === "failed") {
              reject(new Error("failed"));
              return;
            }
            if (attempts >= pollMax) {
              reject(new Error("timeout"));
              return;
            }
            setTimeout(tick, pollMs);
          })
          .catch(function (e) {
            if (attempts >= pollMax) reject(e);
            else setTimeout(tick, pollMs);
          });
      }
      tick();
    });
  }

  function syncPurchase(purchaseId) {
    return fetch("/api/noms/purchase/" + encodeURIComponent(String(purchaseId)) + "/sync", {
      method: "POST",
      headers: csrfHeaders(),
      credentials: "same-origin",
    }).then(function (r) {
      if (!r.ok) return null;
      return r.json();
    });
  }

  function finishSuccessfulPayment(purchaseId, grantedNoms, data) {
    if (!data) return;
    updateTopbarBalance(data.balance_after);
    activePurchaseId = null;
    payInFlight = false;
    var toastTpl = i18n(
      "noms_purchase_success",
      "Payment successful — {noms} Noms added to your wallet."
    );
    var toast = toastTpl.replace("{noms}", String(data.noms_granted || grantedNoms));
    closeModal();
    showPurchaseNotify(toast, 3600);
  }

  function restorePaymentAfterFailure(message) {
    payInFlight = false;
    setProcessingState(false);
    showError(message);
    updatePayButtonState();
  }

  function paymentErrorMessage(err) {
    if (!err) {
      return i18n("noms_purchase_failed", "Payment failed. Check your card details and try again.");
    }
    if (typeof err === "string") return err;
    if (err.message) return String(err.message);
    return i18n("noms_purchase_failed", "Payment failed. Check your card details and try again.");
  }

  function submitPayment(purchaseId, grantedNoms) {
    if (payInFlight || !activeStripe || !activeElements || !activeClientSecret || !cardComplete) {
      return;
    }
    payInFlight = true;
    updatePayButtonState();
    setProcessingState(
      true,
      "noms_purchase_stage_contacting_stripe",
      "Contacting Stripe…"
    );

    activeElements
      .submit()
      .then(function (submitResult) {
        if (submitResult.error) {
          throw submitResult.error;
        }
        setProcessingState(
          true,
          "noms_purchase_stage_processing_payment",
          "Processing payment…"
        );
        return activeStripe.confirmPayment({
          elements: activeElements,
          clientSecret: activeClientSecret,
          confirmParams: confirmPaymentParams(),
          redirect: "if_required",
        });
      })
      .then(function (result) {
        if (result.error) {
          throw result.error;
        }
        var pi = result.paymentIntent;
        if (pi && pi.status && pi.status !== "succeeded" && pi.status !== "processing") {
          throw new Error(
            i18n("noms_purchase_failed", "Payment failed. Check your card details and try again.")
          );
        }
        setProcessingState(
          true,
          "noms_purchase_stage_crediting_noms",
          "Crediting Noms…"
        );
        return syncPurchase(purchaseId).then(function (syncData) {
          if (syncData && syncData.status === "completed" && typeof syncData.balance_after === "number") {
            return syncData;
          }
          return pollPurchase(purchaseId);
        });
      })
      .then(function (data) {
        if (!data) {
          throw new Error(
            i18n("noms_purchase_failed", "Payment failed. Check your card details and try again.")
          );
        }
        finishSuccessfulPayment(purchaseId, grantedNoms, data);
      })
      .catch(function (err) {
        if (err && err.message === "failed") {
          restorePaymentAfterFailure(
            i18n("noms_purchase_failed", "Payment failed. Check your card details and try again.")
          );
          return;
        }
        if (err && err.message === "timeout") {
          restorePaymentAfterFailure(
            i18n(
              "noms_purchase_pending",
              "Payment received — refresh if your balance lags."
            )
          );
          return;
        }
        restorePaymentAfterFailure(paymentErrorMessage(err));
      });
  }

  function startCheckout(config) {
    var pack = selectedPack();
    if (!pack || !pack.can_purchase) {
      teardownCheckout();
      return;
    }
    if (checkoutInFlight) return;
    checkoutInFlight = true;
    showError("");
    var priorPurchaseId = activePurchaseId;
    activePurchaseId = null;
    teardownCheckout();
    setPaymentWrapVisible(false);

    var beginCheckout = function () {
      loadStripeJs()
        .then(function () {
          return fetch("/api/noms/purchase/intent", {
            method: "POST",
            headers: csrfHeaders(),
            credentials: "same-origin",
            body: JSON.stringify({ sku: pack.sku }),
          });
        })
        .then(function (r) {
          return r.json().then(function (body) {
            if (!r.ok) {
              throw new Error((body && body.error) || "intent failed");
            }
            return body;
          });
        })
        .then(function (intent) {
          activePurchaseId = intent.purchase_id;
          activeClientSecret = intent.client_secret;
          activeStripe = window.Stripe(config.publishable_key);
          activeElements = activeStripe.elements({
            clientSecret: intent.client_secret,
            appearance: stripeAppearance(),
          });
          activePayment = activeElements.create(
            "payment",
            paymentElementOptions(config)
          );

          var purchaseId = intent.purchase_id;
          var grantedNoms = pack.noms;
          var payBtn = document.getElementById("nom-purchase-pay");
          if (payBtn) {
            payBtn.onclick = function () {
              submitPayment(purchaseId, grantedNoms);
            };
          }

          cardComplete = false;
          activePayment.on("change", function (ev) {
            cardComplete = !!(ev && ev.complete);
            if (ev && ev.error && ev.error.message) {
              showError(ev.error.message);
            } else if (cardComplete) {
              showError("");
            }
            updatePayButtonState();
          });

          activePayment.on("ready", function () {
            updatePayButtonState();
          });

          setPaymentWrapVisible(true);
          updateStripeNote();
          var mount = document.getElementById("nom-purchase-payment");
          if (mount) {
            requestAnimationFrame(function () {
              if (activePayment) activePayment.mount(mount);
            });
          }
          updatePayButtonState();
          checkoutInFlight = false;
        })
        .catch(function (e) {
          checkoutInFlight = false;
          showError((e && e.message) || i18n("noms_purchase_failed", "Could not start checkout."));
        });
    };
    if (priorPurchaseId) {
      cancelPendingPurchase(priorPurchaseId).then(beginCheckout);
    } else {
      beginCheckout();
    }
  }

  function onWalletClick(ev) {
    ev.preventDefault();
    fetch("/api/noms/purchase/config", {
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    })
      .then(function (r) {
        if (!r.ok) throw new Error("config");
        return r.json();
      })
      .then(function (config) {
        openModal(config);
      })
      .catch(function () {
        openModal({ enabled: false, balance: 0, packs: [] });
      });
  }

  function init() {
    document.addEventListener("click", function (ev) {
      var btn =
        ev.target && ev.target.closest
          ? ev.target.closest("#app-topbar-noms-btn")
          : null;
      if (!btn) return;
      onWalletClick(ev);
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();

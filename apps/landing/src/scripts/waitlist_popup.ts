// Waitlist popup runtime: read `?waitlist=` from the URL, reveal the
// matching popup variant, and strip the param so a refresh does not
// re-trigger the popup.

type PopupState = "success" | "oauth_denied" | "oauth_failed" | "state_invalid" | "server_error";

const VALID_REASONS: readonly Exclude<PopupState, "success">[] = [
  "oauth_denied",
  "oauth_failed",
  "state_invalid",
  "server_error",
];

const readQueryState = (search: string): PopupState | undefined => {
  const params = new URLSearchParams(search);
  const value = params.get("waitlist");
  if (value === "success") {
    return "success";
  }
  if (value === "error") {
    const reason = params.get("reason");
    if (reason !== null && (VALID_REASONS as readonly string[]).includes(reason)) {
      return reason as PopupState;
    }
  }
  return undefined;
};

const copyTemplateInto = (
  state: PopupState,
  titleEl: HTMLElement,
  bodyEl: HTMLElement,
): boolean => {
  const template = document.querySelector<HTMLTemplateElement>(
    `template[data-waitlist-copy="${state}"]`,
  );
  if (!template) {
    return false;
  }
  const titleSlot = template.content.querySelector<HTMLElement>("[data-waitlist-copy-title]");
  const bodySlot = template.content.querySelector<HTMLElement>("[data-waitlist-copy-body]");
  if (!titleSlot || !bodySlot) {
    return false;
  }
  titleEl.textContent = titleSlot.textContent ?? "";
  bodyEl.textContent = bodySlot.textContent ?? "";
  return true;
};

const bucketState = (state: PopupState): "success" | "neutral" | "error" => {
  if (state === "success") {
    return "success";
  }
  if (state === "oauth_denied") {
    return "neutral";
  }
  return "error";
};

const init = (): void => {
  const state = readQueryState(globalThis.location.search);
  if (state === undefined) {
    return;
  }

  const popup = document.querySelector<HTMLElement>("[data-waitlist-popup]");
  if (!popup) {
    return;
  }

  const titleEl = popup.querySelector<HTMLElement>("[data-waitlist-title]");
  const bodyEl = popup.querySelector<HTMLElement>("[data-waitlist-body]");
  const iconEl = popup.querySelector<HTMLElement>("[data-waitlist-icon]");
  const closeBtn = popup.querySelector<HTMLButtonElement>("[data-waitlist-close]");

  if (!titleEl || !bodyEl || !iconEl || !closeBtn) {
    return;
  }

  if (!copyTemplateInto(state, titleEl, bodyEl)) {
    return;
  }

  const variant = bucketState(state);
  popup.dataset["state"] = "visible";
  iconEl.dataset["state"] = variant;

  closeBtn.addEventListener("click", () => {
    popup.dataset["state"] = "hidden";
  });

  // History.replaceState wipes the query so a refresh does not re-trigger.
  history.replaceState(null, "", globalThis.location.pathname);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";

type Clip = { id: number; text: string; kind: "text" | "image" | "files"; preview: string | null };
type Snapshot = { items: Clip[]; paused: boolean; error: string | null; gnome: boolean; shortcut: boolean; autostart: boolean; mac: boolean; paste_permission: boolean; paste_target: boolean };
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const search = element<HTMLInputElement>("search");
const clips = element<HTMLUListElement>("clips");
const pause = element<HTMLButtonElement>("pause");
const clear = element<HTMLButtonElement>("clear");
const autostart = element<HTMLInputElement>("autostart");
let state: Snapshot | undefined;
let toastTimer: ReturnType<typeof setTimeout>;
let refreshGeneration = 0;
let selecting = false;

async function choose(id: number, copyOnly: boolean) {
  if (selecting) return;
  selecting = true;
  try {
    const result = await invoke<string>("select_clip", { id, copyOnly });
    if (result === "permission") toast("Copied. Use ••• → Enable direct paste to allow Accessibility, then reopen from your destination app.");
    else if (result === "copied") toast("Copied.");
    await refresh();
  } catch (error) { toast(String(error)); }
  finally { selecting = false; }
}

function toast(message: string) {
  const node = element("toast");
  node.textContent = message;
  node.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { node.hidden = true; }, 2500);
}

async function perform(command: string, args?: Record<string, unknown>) {
  try { await invoke(command, args); await refresh(); }
  catch (error) { toast(String(error)); }
}

function render() {
  if (!state) return;
  const query = search.value.toLocaleLowerCase();
  const visible = state.items.filter((clip) => clip.text.toLocaleLowerCase().includes(query));
  const focusedId = (document.activeElement as HTMLElement)?.closest<HTMLButtonElement>(".clip")?.dataset.id;
  clips.replaceChildren();
  visible.forEach((clip) => {
    const row = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "clip";
    button.dataset.id = String(clip.id);
    const text = document.createElement("span");
    text.className = "clip-text";
    text.dir = "auto";
    // Clipboard content is untrusted text. Never interpret HTML or build links from it.
    text.textContent = clip.text.slice(0, 400);
    if (clip.kind !== "text") {
      const preview = document.createElement(clip.preview ? "img" : "span");
      preview.className = "clip-preview";
      if (preview instanceof HTMLImageElement) { preview.src = clip.preview!; preview.alt = ""; }
      else { preview.textContent = clip.kind === "files" ? "▤" : "▧"; preview.setAttribute("aria-hidden", "true"); }
      button.append(preview);
      button.setAttribute("aria-label", `${clip.kind === "files" ? "Files" : "Image"}: ${clip.text.slice(0, 400)}`);
    }
    button.append(text);
    button.addEventListener("click", (event) => choose(clip.id, event.shiftKey));
    row.append(button);
    clips.append(row);
    if (focusedId === String(clip.id)) button.focus({ preventScroll: true });
  });
  element("count").textContent = `${state.items.length} / 30 clips`;
  element("activity").textContent = state.paused ? "Ⅱ Paused" : state.error ? "○ Needs attention" : "● Listening";
  element("activity").classList.toggle("paused", state.paused || !!state.error);
  pause.textContent = state.paused ? "Resume" : "Pause";
  pause.setAttribute("aria-pressed", String(state.paused));
  clear.disabled = state.items.length === 0;
  autostart.checked = state.autostart;
  const notice = element("notice");
  notice.textContent = state.error;
  notice.hidden = !state.error;
  element("gnome").hidden = !state.gnome || !state.error;
  const empty = element("empty");
  empty.hidden = visible.length > 0;
  empty.querySelector("h2")!.textContent = query ? "No matching clips" : state.paused ? "History paused" : "No clips yet";
  empty.querySelector("p")!.textContent = query ? "Try another search." : state.paused ? "Resume to capture new copies." : "Copy text, an image, or files to start.";
  element("shortcut").hidden = !state.shortcut;
  element("shortcut").textContent = state.mac ? "⌘⇧V" : "Ctrl Alt V";
  element("paste-permission").hidden = !state.mac || state.paste_permission;
  element("paste-hint").textContent = !state.paste_permission ? "Click to copy · Enable direct paste in •••" :
    state.paste_target ? "Click to paste · Shift-click to copy" : "Open from your destination app to paste";
}

async function refresh() {
  const generation = ++refreshGeneration;
  try {
    const next = await invoke<Snapshot>("snapshot");
    if (generation !== refreshGeneration) return;
    state = next;
    render();
  } catch (error) { if (generation === refreshGeneration) toast(String(error)); }
}

search.addEventListener("input", render);
pause.addEventListener("click", () => perform("toggle_pause"));
clear.addEventListener("click", () => perform("clear_history"));
autostart.addEventListener("change", () => perform("set_autostart", { enabled: autostart.checked }));
element("quit").addEventListener("click", () => perform("quit"));
element("close").addEventListener("click", () => perform("close_window"));
element("paste-permission").addEventListener("click", () => perform("request_paste_permission"));
element("gnome").addEventListener("click", () => perform("enable_gnome"));
document.addEventListener("keydown", (event) => {
  const settings = element<HTMLDetailsElement>("settings");
  if (event.key === "Escape") {
    if (settings.open) { settings.open = false; settings.querySelector("summary")?.focus(); }
    else { void invoke("close_window"); }
  }
  if (event.key === "Enter" && !event.repeat) {
    const button = document.activeElement === search ? clips.querySelector<HTMLButtonElement>(".clip") :
      (document.activeElement as HTMLElement)?.closest<HTMLButtonElement>(".clip");
    if (button) { event.preventDefault(); void choose(Number(button.dataset.id), event.shiftKey); }
  }
  if ((event.key === "ArrowDown" || event.key === "ArrowUp") &&
      (document.activeElement === search || (document.activeElement as HTMLElement)?.classList.contains("clip"))) {
    const buttons = Array.from(clips.querySelectorAll<HTMLButtonElement>("button"));
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (buttons.length) {
      event.preventDefault();
      const next = index < 0 ? event.key === "ArrowDown" ? 0 : buttons.length - 1 :
        (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next].focus();
    }
  }
  if ((event.metaKey || event.ctrlKey) && event.key === "f") { event.preventDefault(); search.focus(); }
});
await listen("history-changed", refresh);
await refresh();
search.focus();

// End-to-end acceptance in the real webview, against an in-memory test clipboard.
const smokeStage = await invoke<number>("test_mode");
if (smokeStage === 2) {
  const reopened = await invoke<Snapshot>("snapshot");
  await invoke("finish_smoke_test", { success: reopened.paused && !reopened.items.length && !clips.children.length && !element("empty").hidden });
} else if (smokeStage === 1) {
  try {
    const renderedCount = () => clips.children.length;
    const initial = await invoke<Snapshot>("snapshot");
    if (initial.items.length !== 5 || renderedCount() !== 5) throw new Error("history did not render");
    const image = clips.querySelector("img");
    if (!image || !initial.items.some(c => c.kind === "files" && c.text.includes("FeatherClip demo.txt"))) throw new Error("image/file entries did not render");
    await image.decode();
    search.focus();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
    if ((document.activeElement as HTMLElement)?.dataset.id !== String(initial.items[4].id)) throw new Error("keyboard navigation failed");
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    if ((document.activeElement as HTMLElement)?.dataset.id !== String(initial.items[0].id)) throw new Error("keyboard wrap failed");
    const settings = element<HTMLDetailsElement>("settings");
    settings.open = true;
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    if (settings.open) throw new Error("options did not dismiss");
    if (document.documentElement.scrollWidth > window.innerWidth || document.documentElement.scrollHeight > window.innerHeight) throw new Error("panel overflow");
    search.value = "سلام";
    render();
    if (renderedCount() !== 1) throw new Error("Unicode search failed");
    search.value = "";
    const selection = await invoke<string>("select_clip", { id: initial.items[2].id, copyOnly: false });
    if (selection !== "copied") throw new Error("synthetic mode attempted real paste");
    const copied = await invoke<Snapshot>("snapshot");
    if (copied.items[0].id !== initial.items[2].id) throw new Error("copy failed");
    await invoke("toggle_pause");
    if (!(await invoke<Snapshot>("snapshot")).paused) throw new Error("pause failed");
    await invoke("clear_history");
    await refresh();
    if (clips.children.length || element("empty").hidden) throw new Error("clear failed");
    await invoke("finish_smoke_test", { success: true });
  } catch {
    await invoke("finish_smoke_test", { success: false });
  }
}

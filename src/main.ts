import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";

type Clip = { id: number; text: string };
type Snapshot = { items: Clip[]; paused: boolean; error: string | null; gnome: boolean; shortcut: boolean; autostart: boolean; mac: boolean };
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const search = element<HTMLInputElement>("search");
const clips = element<HTMLUListElement>("clips");
const pause = element<HTMLButtonElement>("pause");
const clear = element<HTMLButtonElement>("clear");
const autostart = element<HTMLInputElement>("autostart");
let state: Snapshot | undefined;
let toastTimer: ReturnType<typeof setTimeout>;
let refreshGeneration = 0;

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
    button.append(text);
    button.addEventListener("click", async () => {
      try { await invoke("copy_clip", { id: clip.id }); toast("Copied. Ready to paste."); await refresh(); }
      catch (error) { toast(String(error)); }
    });
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
  empty.querySelector("p")!.textContent = query ? "Try another search." : state.paused ? "Resume to capture new copies." : "Copy some text to start.";
  element("shortcut").hidden = !state.shortcut;
  element("shortcut").textContent = state.mac ? "⌘⇧V" : "Ctrl Alt V";
  element("paste-hint").textContent = `Click to copy · ${state.mac ? "⌘V" : "Ctrl+V"} to paste`;
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
element("gnome").addEventListener("click", () => perform("enable_gnome"));
document.addEventListener("keydown", (event) => {
  const settings = element<HTMLDetailsElement>("settings");
  if (event.key === "Escape") {
    if (settings.open) { settings.open = false; settings.querySelector("summary")?.focus(); }
    else { void invoke("close_window"); }
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
if (await invoke<boolean>("test_mode")) {
  try {
    const renderedCount = () => clips.children.length;
    const initial = await invoke<Snapshot>("snapshot");
    if (initial.items.length !== 3 || renderedCount() !== 3) throw new Error("history did not render");
    search.value = "سلام";
    render();
    if (renderedCount() !== 1) throw new Error("Unicode search failed");
    search.value = "";
    await invoke("copy_clip", { id: initial.items[2].id });
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

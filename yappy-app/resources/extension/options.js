const tokenEl = document.getElementById("token");
const saveEl = document.getElementById("save");
const savedEl = document.getElementById("saved");

chrome.storage.local.get("yappyToken").then(({ yappyToken }) => {
  if (yappyToken) tokenEl.value = yappyToken;
});

saveEl.addEventListener("click", async () => {
  const t = (tokenEl.value || "").trim();
  if (!t) return;
  await chrome.storage.local.set({ yappyToken: t });
  chrome.runtime.sendMessage({ type: "reconnect" });
  savedEl.classList.add("show");
  setTimeout(() => savedEl.classList.remove("show"), 1500);
});

tokenEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter") saveEl.click();
});

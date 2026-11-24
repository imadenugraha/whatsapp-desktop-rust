(function () {
  if (location.hostname !== "web.whatsapp.com") return;
  var post = function (o) { try { window.ipc.postMessage(JSON.stringify(o)); } catch (e) {} };

  // Native desktop notifications: replace Web Notification API.
  function N(title, opts) {
    this.title = title;
    this.onclick = this.onclose = this.onerror = this.onshow = null;
    post({ t: "notify", title: String(title || ""), body: String((opts && opts.body) || "") });
  }
  N.permission = "granted";
  N.requestPermission = function (cb) { if (cb) cb("granted"); return Promise.resolve("granted"); };
  N.prototype.close = function () {};
  N.prototype.addEventListener = N.prototype.removeEventListener = function () {};
  Object.defineProperty(window, "Notification", { value: N, configurable: true, writable: true });

  // Permissions API: report notifications as granted.
  try {
    var q = navigator.permissions && navigator.permissions.query.bind(navigator.permissions);
    if (q) navigator.permissions.query = function (d) {
      if (d && d.name === "notifications") return Promise.resolve({ state: "granted", onchange: null });
      return q(d);
    };
  } catch (e) {}

  // Service-worker notifications go through the same path.
  try {
    if (window.ServiceWorkerRegistration) {
      ServiceWorkerRegistration.prototype.showNotification = function (title, opts) {
        new N(title, opts); return Promise.resolve();
      };
    }
  } catch (e) {}

  // Image paste: WebKitGTK does not expose clipboard bitmaps to the page, so
  // ask the native side for the clipboard image and replay it as a paste event.
  // Only runs on a user paste (Ctrl+V / menu) that carries no files or text.
  var pasteTarget = null;
  document.addEventListener("paste", function (e) {
    var cd = e.clipboardData;
    if (!e.isTrusted || !cd || (cd.files && cd.files.length)) return;
    if (cd.getData("text/plain")) return;
    var html = cd.getData("text/html");
    if (html && !/<img/i.test(html)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    pasteTarget = document.activeElement || document.body;
    post({ t: "paste_image" });
  }, true);
  window.__wdPaste = function (list) {
    try {
      var dt = new DataTransfer();
      list.forEach(function (f) {
        var bin = atob(f.data), bytes = new Uint8Array(bin.length);
        for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        dt.items.add(new File([bytes], f.name, { type: f.type }));
      });
      var t = pasteTarget && document.contains(pasteTarget) ? pasteTarget : (document.activeElement || document.body);
      pasteTarget = null;
      t.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
    } catch (e) {}
  };

  // Unread count in window title.
  var last = "";
  function title() {
    var t = document.title || "";
    if (t !== last) { last = t; post({ t: "title", title: t }); }
  }
  setInterval(title, 2000);
})();

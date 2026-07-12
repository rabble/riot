public enum RiotJS {
    /// Injected at document start. Defines window.riot over the
    /// webkit message handler with promise-correlation ids. The host
    /// resolves calls via window.__riotResolve and pushes change events
    /// via window.__riotDataChanged.
    public static let source = """
    (function () {
      const pending = new Map();
      let nextId = 1;
      const watchers = [];
      function call(op, params) {
        return new Promise((resolve, reject) => {
          const id = nextId++;
          pending.set(id, { resolve, reject });
          window.webkit.messageHandlers.riot.postMessage(Object.assign({ id, op }, params));
        });
      }
      window.__riotResolve = function (id, ok, payload) {
        const entry = pending.get(id);
        if (!entry) { return; }
        pending.delete(id);
        if (ok) { entry.resolve(payload); } else { entry.reject(new Error(String(payload))); }
      };
      window.__riotDataChanged = function () {
        for (const watcher of watchers) {
          window.riot.list(watcher.prefix).then(watcher.cb).catch(function () {});
        }
      };
      window.riot = {
        get: function (key) {
          return call("get", { key: key }).then(function (v) { return v == null ? null : JSON.parse(v); });
        },
        put: function (key, value) {
          return call("put", { key: key, value: JSON.stringify(value) }).then(function () { return undefined; });
        },
        list: function (prefix) {
          // Prefixes are segment-based; a trailing "/" would produce an
          // empty segment the core rejects, so normalize it away here.
          var clean = prefix.replace(/\\/+$/, "");
          return call("list", { prefix: clean }).then(function (rows) {
            return rows.map(function (r) { return { key: r.key, value: JSON.parse(r.value) }; });
          });
        },
        watch: function (prefix, cb) {
          watchers.push({ prefix: prefix, cb: cb });
          window.riot.list(prefix).then(cb).catch(function () {});
        },
        // Returns { id, displayName, tag }. Store the ID. displayName and tag
        // are only what to draw right now — re-resolve them with riot.profile()
        // at render time, or a rename can never repair the rows already written.
        whoami: function () { return call("whoami", {}); },
        // Resolves a stored id to { displayName, tag }. Render as
        // displayName + " · " + tag; the host guarantees displayName carries no
        // separator. An id with no profile yet is not an error — it comes back
        // as the "member" fallback. Note the param travels as "subject", not
        // "id": "id" is this envelope's promise-correlation key.
        profile: function (id) { return call("profile", { subject: String(id) }); },
      };
    })();
    """
}

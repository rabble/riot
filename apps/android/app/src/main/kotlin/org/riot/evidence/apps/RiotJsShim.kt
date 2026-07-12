package org.riot.evidence.apps

/**
 * The `window.riot` document-start shim. Keeps the API byte-identical to
 * iOS's shim from the app's point of view (`get`/`put`/`list`/`watch`/
 * `whoami`, Promises, trailing-slash prefix normalization) so
 * `fixtures/apps/checklist/app.js` runs unmodified. It wraps the
 * synchronous `RiotNative` @JavascriptInterface, parsing each JSON envelope
 * into a resolved value or a rejected Error.
 */
object RiotJsShim {
    // Not `const`: the ${'$'} escape below makes this a non-constant expression.
    val SOURCE = """
    (function () {
      if (window.riot) { return; }
      var watchers = [];
      function call(fn) {
        return new Promise(function (resolve, reject) {
          var envelope;
          try { envelope = JSON.parse(fn()); } catch (e) { reject(new Error("bridge unavailable")); return; }
          if (envelope.ok) { resolve(envelope.value); } else { reject(new Error(String(envelope.error))); }
        });
      }
      function fireWatchers() {
        watchers.forEach(function (w) {
          window.riot.list(w.prefix).then(w.cb).catch(function () {});
        });
      }
      window.__riotDataChanged = fireWatchers;
      window.riot = {
        get: function (key) {
          return call(function () { return RiotNative.riotGet(String(key)); })
            .then(function (v) { return v == null ? null : JSON.parse(v); });
        },
        put: function (key, value) {
          return call(function () { return RiotNative.riotPut(String(key), JSON.stringify(value)); })
            .then(function () { fireWatchers(); });
        },
        list: function (prefix) {
          var clean = String(prefix).replace(/\/+${'$'}/, "");
          return call(function () { return RiotNative.riotList(clean); })
            .then(function (rows) {
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
        whoami: function () { return call(function () { return RiotNative.riotWhoami(); }); },
        // Resolves a stored id to { displayName, tag }. Render as
        // displayName + " · " + tag; the host guarantees displayName carries no
        // separator. An id with no profile yet is not an error — it comes back
        // as the "member" fallback.
        profile: function (id) { return call(function () { return RiotNative.riotProfile(String(id)); }); },
      };
    })();
    """
}

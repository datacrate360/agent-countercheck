// Live UI script for `countercheck ui`.
//
// Polls /state every 2 seconds (with the page's token) and reloads the page when a receipt is
// added, removed or rewritten, keeping the runs you had open still open and the filters you picked. It talks only to the
// page's own server: the Content-Security-Policy allows nothing else, and the browser runs this
// script only because it carries the page's one-time nonce.
//
// Compiled into the countercheck binary with include_str!; no build step, no dependencies.
// Keep it plain browser JavaScript and free of the closing script tag text.
(function () {
  var token = new URLSearchParams(location.search).get('t') || '';
  var storageKey = 'countercheck-open';
  var last = null;
  var status = document.getElementById('live');

  // Restore which runs were open before the last reload.
  try {
    var open = JSON.parse(sessionStorage.getItem(storageKey) || 'null');
    if (open) {
      document.querySelectorAll('details.run').forEach(function (d) {
        d.open = open.indexOf(d.id) >= 0;
        // A run that was already open doesn't replay its steps on every reload.
        var steps = d.querySelector('.steps');
        if (d.open && steps) { steps.classList.add('now'); }
      });
    }
  } catch (e) { /* no storage: keep the default */ }

  function saveOpenRuns() {
    try {
      var ids = Array.prototype.map.call(
        document.querySelectorAll('details.run[open]'),
        function (d) { return d.id; }
      );
      sessionStorage.setItem(storageKey, JSON.stringify(ids));
    } catch (e) { /* ignore */ }
  }
  document.addEventListener('toggle', saveOpenRuns, true);

  // Keep the chosen filters (verdict, agent) across reloads too.
  var filterKey = 'countercheck-filters';
  try {
    JSON.parse(sessionStorage.getItem(filterKey) || '[]').forEach(function (id) {
      var input = document.getElementById(id);
      if (input) { input.checked = true; }
    });
  } catch (e) { /* no storage: keep the defaults */ }
  document.addEventListener('change', function () {
    try {
      var ids = Array.prototype.map.call(
        document.querySelectorAll('.filters input:checked'),
        function (i) { return i.id; }
      );
      sessionStorage.setItem(filterKey, JSON.stringify(ids));
    } catch (e) { /* ignore */ }
  });

  // Flow map: draw only lines that are new since the last reload (a new agent, or an agent whose
  // latest verdict changed), and light the grid around the pointer.
  var flow = document.querySelector('.flow');
  if (flow) {
    var drawnKey = 'countercheck-drawn';
    var drawn = [];
    try { drawn = JSON.parse(sessionStorage.getItem(drawnKey) || '[]'); } catch (e) { drawn = []; }
    var now = [];
    flow.querySelectorAll('.fn').forEach(function (node) {
      var m = node.className.match(/\ba-(\d+)\b/);
      var name = node.querySelector('b');
      if (!m || !name) { return; }
      var key = name.textContent + '|' + node.className.replace(/\b(a|x)-\d+\b/g, '').trim();
      now.push(key);
      if (drawn.indexOf(key) >= 0) {
        flow.querySelectorAll('.fe.a-' + m[1] + ',.pulse.a-' + m[1]).forEach(function (el) {
          el.classList.add('seen');
        });
      }
    });
    try { sessionStorage.setItem(drawnKey, JSON.stringify(now)); } catch (e) { /* ignore */ }
    flow.addEventListener('pointermove', function (e) {
      var r = flow.getBoundingClientRect();
      flow.style.setProperty('--x', (e.clientX - r.left) + 'px');
      flow.style.setProperty('--y', (e.clientY - r.top) + 'px');
    });
    flow.addEventListener('pointerleave', function () {
      flow.style.removeProperty('--x');
      flow.style.removeProperty('--y');
    });
  }

  function poll() {
    fetch('/state?t=' + encodeURIComponent(token), { cache: 'no-store' })
      .then(function (r) {
        if (!r.ok) { throw new Error('status ' + r.status); }
        return r.text();
      })
      .then(function (state) {
        if (last !== null && state !== last) {
          saveOpenRuns();
          location.reload();
          return;
        }
        last = state;
        status.textContent = 'live, checked ' + new Date().toLocaleTimeString();
        setTimeout(poll, 2000);
      })
      .catch(function () {
        status.textContent = 'disconnected: is countercheck ui still running?';
        setTimeout(poll, 5000);
      });
  }
  poll();
})();

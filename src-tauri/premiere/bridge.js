/* Same-user, local mailbox. No network listener and no arbitrary script commands. */
(function () {
  'use strict';
  var node = window.cep_node, fs = node.require('fs'), path = node.require('path');
  var root = path.join(node.process.env.APPDATA, 'dev.dean.container', 'premiere-bridge');
  var busy = false, statusStarted = 0, generation = 0, transferring = false, statusRetries = 0;
  function read(name) { try { return JSON.parse(fs.readFileSync(path.join(root, name), 'utf8')); } catch (_) { return null; } }
  function write(name, value) {
    var temp = path.join(root, name + '.tmp');
    fs.writeFileSync(temp, JSON.stringify(value)); fs.renameSync(temp, path.join(root, name));
  }
  function literal(value) { return JSON.stringify(value).replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029'); }
  function tick() {
    if (busy) {
      // Only read-only status queries may be retried. Never replay an insertion.
      if (transferring || Date.now() - statusStarted < 5000) return;
      if (statusRetries >= 1) return; // Never accumulate an unbounded queue on a hung host.
      statusRetries++;
      busy = false; generation++;
    }
    var session = read('session.json');
    if (!session || typeof session.id !== 'string' || Date.now() - session.updated > 7000 || session.updated > Date.now() + 1000) return;
    busy = true; statusStarted = Date.now();
    var token = ++generation;
    window.__adobe_cep__.evalScript('ContainerPremiere.status()', function (raw) {
      if (token !== generation) return; // Ignore a late answer to an abandoned query.
      statusRetries = 0;
      try {
        var host = JSON.parse(raw);
        write('heartbeat.json', {session: session.id, updated: Date.now(), connected: host.connected === true, protocol: host.protocol || 0, project: host.project || '', sequence_id: host.sequence_id || '', sequence_name: host.sequence_name || ''});
        var request = read('request.json');
        if (!host.connected || !request || request.session !== session.id || !/^[a-f0-9]{64}$/.test(request.id)) { busy = false; return; }
        // Claim once, before entering Premiere. Expired requests are never replayed on reconnect.
        fs.unlinkSync(path.join(root, 'request.json'));
        if (request.expires < Date.now() || request.expires > Date.now() + 31000 || request.operation !== 'append' || typeof request.path !== 'string' || !/\.(mp4|mov|mkv|webm|avi|m4v)$/i.test(request.path) || typeof request.project !== 'string' || typeof request.sequence_id !== 'string') { busy = false; return; }
        if ((!request.project || request.separate_project === true) && (typeof request.project_path !== 'string' || !/\.prproj$/i.test(request.project_path))) { busy = false; return; }
        if (typeof request.has_audio !== 'boolean') { busy = false; return; }
        transferring = true;
        var separate = request.separate_project === true;
        var script = separate
          ? 'ContainerPremiere.prepare(' + literal(request.path) + ',' + literal(request.project) + ',' + literal(request.sequence_id) + ',' + literal(request.expires) + ',' + literal(request.project_path) + ',' + literal(request.has_audio) + ',' + literal(request.id) + ')'
          : 'ContainerPremiere.insert(' + literal(request.path) + ',' + literal(request.project) + ',' + literal(request.sequence_id) + ',' + literal(request.expires) + ',' + literal(request.project_path || '') + ',' + literal(request.has_audio) + ',false)';
        function record(answer) {
          try {
            var result = JSON.parse(answer);
            if (result.created_project_path) {
              // Cleanup bookkeeping must never hide an already completed transfer.
              try {
                var ledgerPath = path.join(root, 'created-projects.json');
                var projects = fs.existsSync(ledgerPath) ? JSON.parse(fs.readFileSync(ledgerPath, 'utf8')) : [];
                if (!Array.isArray(projects)) throw new Error('Invalid ownership list');
                if (projects.indexOf(result.created_project_path) < 0) projects.push(result.created_project_path);
                write('created-projects.json', projects);
              } catch (_) { result.ownership_record_failed = true; }
            }
            write('response.json', {session: session.id, id: request.id, ok: result.ok === true, code: result.code || 'host_error', project_id: result.project_id || '', completed_at: Date.now(), stage: result.stage || '', timings: result.timings || [], ownership_record_failed: result.ownership_record_failed === true});
          } catch (_) {}
          transferring = false; busy = false;
        }
        window.__adobe_cep__.evalScript(script, function (answer) {
          try {
            var result = JSON.parse(answer);
            if (separate && result.ok && result.code === 'project_prepared') {
              // Yield back to Premiere after project/workspace creation before importing media.
              setTimeout(function () {
                window.__adobe_cep__.evalScript('ContainerPremiere.finish(' + literal(request.id) + ')', record);
              }, 250);
              return;
            }
          } catch (_) {}
          record(answer);
        });
      } catch (_) { busy = false; }
    });
  }
  setInterval(tick, 1000); tick();
}());

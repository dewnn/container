/* Original CONTAINER bridge. Public Premiere APIs only; no QE DOM. */
var ContainerPremiere = (function () {
    function quote(value) {
        return '"' + String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\r/g, '\\r').replace(/\n/g, '\\n').replace(/\t/g, '\\t').replace(/[\u0000-\u001f\u2028\u2029]/g, function (c) { return '\\u' + ('0000' + c.charCodeAt(0).toString(16)).slice(-4); }) + '"';
    }
    var createdProjectPath = '', pending = null;
    var timings = [], stage = 'validate';
    function measured(name, fn) {
        stage = name;
        var start = new Date().getTime();
        try { return fn(); } finally { timings.push('{"stage":' + quote(name) + ',"ms":' + (new Date().getTime() - start) + '}'); }
    }
    function result(ok, code) {
        var projectID = '';
        try { projectID = String(app.project.documentID || ''); } catch (_) {}
        return '{"ok":' + (ok ? 'true' : 'false') + ',"code":' + quote(code) + ',"project_id":' + quote(projectID) + ',"stage":' + quote(stage) + ',"timings":[' + timings.join(',') + '],"created_project_path":' + quote(createdProjectPath) + '}';
    }
    function status() {
        try {
            app.setExtensionPersistent('dev.dean.container.premiere.bridge', 1);
            var project = app.project, sequence = project && project.activeSequence;
            return '{"connected":true,"protocol":3,"project":' + quote(project ? project.documentID : '') + ',"sequence_id":' + quote(sequence ? sequence.sequenceID : '') + ',"sequence_name":' + quote(sequence ? sequence.name : '') + '}';
        } catch (e) { return '{"connected":false}'; }
    }
    function find(item, path) {
        if (item.children) for (var i = 0; i < item.children.numItems; i++) {
            var child = item.children[i];
            if (child.type === ProjectItemType.BIN) { var nested = find(child, path); if (nested) return nested; }
            else if (child.getMediaPath && new File(child.getMediaPath()).fsName.toLowerCase() === path.toLowerCase()) return child;
        }
        return null;
    }
    function projectDirectory(fallback) {
        // Read the project MRU only, never the shared import/export directory.
        try {
            var recent = app.properties.getProperty('BE.Prefs.MRU.Document.0');
            if (recent && /\.prproj$/i.test(recent)) {
                recent = String(recent).replace(/^\\\\\?\\/, '');
                var parent = new File(recent).parent;
                if (parent && parent.exists) return parent;
            }
        } catch (_) {}
        try {
            var profile = new Folder(app.getPProPrefPath);
            if (profile.exists && /^Profile-/i.test(profile.name) && profile.parent.exists) return profile.parent;
        } catch (_) {}
        return new File(fallback).parent;
    }
    function insert(path, projectID, sequenceID, expires, projectPath, hasAudio, separateProject, prepareToken, continuationPath) {
        createdProjectPath = continuationPath || '';
        timings = []; stage = 'validate';
        try {
            if (new Date().getTime() > expires) return result(false, 'expired');
            var project = app.project, sequence = project && project.activeSequence;
            if (String(project && project.documentID ? project.documentID : '') !== projectID || String(sequence ? sequence.sequenceID : '') !== sequenceID) return result(false, 'sequence_changed');
            if (!separateProject && sequence && (!sequence.videoTracks.numTracks || (hasAudio !== false && !sequence.audioTracks.numTracks))) return result(false, 'no_tracks');
            var file = new File(path);
            if (!file.exists) return result(false, 'missing_file');
            var newProject = separateProject === true || !projectID;
            if (newProject) {
                if (!projectPath || !/\.prproj$/i.test(projectPath)) return result(false, 'invalid_project_path');
                var directory = projectDirectory(projectPath);
                if (!directory || !directory.exists) return result(false, 'project_directory_unavailable');
                var destination = new File(directory.fsName + '/' + new File(projectPath).name);
                if (destination.exists) return result(false, 'project_exists');
                if (!measured('create_project', function () { return app.newProject(destination.fsName); })) return result(false, 'project_create_failed');
                createdProjectPath = destination.fsName;
                project = app.project;
                if (!project || !project.documentID) return result(false, 'project_create_failed');
                // A truthy return is not enough: verify Premiere switched to the requested project.
                if (String(project.documentID) === projectID) return result(false, 'project_switch_failed');
                if (project.path && new File(project.path).fsName.toLowerCase() !== destination.fsName.toLowerCase()) return result(false, 'project_switch_failed');
                if (project.sequences && project.sequences.numSequences === 0 && project.activeSequence) return result(false, 'stale_active_sequence');
                sequence = null;
                if (prepareToken) {
                    pending = {token:prepareToken, path:path, project:String(project.documentID), projectPath:createdProjectPath, hasAudio:hasAudio, expires:expires + 60000, timings:timings.slice(0)};
                    return result(true, 'project_prepared');
                }
            }
            var item = find(project.rootItem, file.fsName);
            if (!item) {
                if (!measured('import', function () { return project.importFiles([file.fsName], true, project.rootItem, false); })) return result(false, 'import_failed');
                item = find(project.rootItem, file.fsName);
            }
            if (!item) return result(false, 'import_failed');
            if (!sequence) {
                // Premiere derives dimensions, frame rate and audio settings from the rendered clip.
                var created = measured('create_sequence', function () { return project.createNewSequenceFromClips('CONTAINER - ' + item.name, [item], project.rootItem); });
                if (!created) return result(false, 'sequence_create_failed');
                var opened = measured('open_sequence', function () { return project.openSequence(created.sequenceID); });
                if (opened === false) return result(false, 'sequence_created_open_failed');
                if (newProject || continuationPath) {
                    try {
                        // Adobe's CEP types declare void; some hosts return numeric zero.
                        var saved = measured('save', function () { return project.save(); });
                        if (saved === false || (typeof saved === 'number' && saved !== 0)) return result(false, 'sequence_created_save_failed');
                    } catch (_) { return result(false, 'sequence_created_save_failed'); }
                }
                return result(true, 'sequence_created');
            }
            // Append at sequence end, never overwrite the current playhead.
            var end = new Time(); end.ticks = String(sequence.end);
            if (hasAudio === false && !sequence.audioTracks.numTracks) {
                var track = sequence.videoTracks[sequence.videoTracks.numTracks - 1];
                var count = track.clips.numItems;
                measured('insert', function () { track.insertClip(item, end.ticks); });
                return result(track.clips.numItems > count, track.clips.numItems > count ? 'inserted' : 'insert_failed');
            }
            if (!measured('insert', function () { return sequence.insertClip(item, end, sequence.videoTracks.numTracks - 1, sequence.audioTracks.numTracks - 1); })) return result(false, 'insert_failed');
            return result(true, 'inserted');
        } catch (e) { return result(false, 'host_error'); }
    }
    function prepare(path, projectID, sequenceID, expires, projectPath, hasAudio, token) {
        if (pending) return result(false, 'transfer_pending');
        return insert(path, projectID, sequenceID, expires, projectPath, hasAudio, true, token);
    }
    function finish(token) {
        if (!pending || pending.token !== token) return result(false, 'invalid_continuation');
        var work = pending; pending = null; // Claim once; a failed finish is never replayed.
        var raw = insert(work.path, work.project, '', work.expires, '', work.hasAudio, false, '', work.projectPath);
        timings = work.timings.concat(timings);
        // Rebuild the result without JSON.parse (older ExtendScript engines).
        return raw.replace(/"timings":\[[^\]]*\]/, '"timings":[' + timings.join(',') + ']');
    }
    return {status: status, insert: insert, prepare: prepare, finish: finish};
}());

// QinAegis Web Client JS
// Provides UI logic and bridges to Rust via custom protocol RPC

document.addEventListener('DOMContentLoaded', function() {
    initNav();
    initConfig();
    initExplore();
    initGenerate();
    initRun();
    loadState();
});

// ---------------------------------------------------------------------------
// RPC Bridge (fetch-based, matches init_script in Rust)
// ---------------------------------------------------------------------------
window.rpc = function(method, params) {
    var controller = null;
    var timeoutId = null;
    return new Promise(function(resolve, reject) {
        var id = Date.now();
        var paramsStr = encodeURIComponent(JSON.stringify(params));
        var url = 'app://localhost/invoke?method=' + encodeURIComponent(method) + '&params=' + paramsStr + '&id=' + id;
        controller = new AbortController();
        timeoutId = setTimeout(function() {
            controller.abort();
            reject(new Error('timeout'));
        }, 60000);
        fetch(url, { signal: controller.signal }).then(function(resp) {
            clearTimeout(timeoutId);
            return resp.text();
        }).then(function(text) {
            try { resolve(JSON.parse(text)); }
            catch(e) { reject(e); }
        }).catch(function(e) {
            clearTimeout(timeoutId);
            reject(e);
        });
    });
};

window.getState = function() { return window.rpc('getState', {}); };
window.setConfig = function(c) { return window.rpc('setConfig', {config: c}); };
window.runExplore = function(url, depth) { return window.rpc('runExplore', {url: url, depth: depth}); };
window.runGenerate = function(req, spec) { return window.rpc('runGenerate', {requirement: req, spec: spec || null}); };
window.runTests = function(project, type) { return window.rpc('runTests', {project: project, type: type}); };
window.getOutput = function() { return window.rpc('getOutput', {}); };
window.clearOutput = function() { return window.rpc('clearOutput', {}); };
window.getProjects = function() { return window.rpc('getProjects', {}); };
console.log('RPC bridge ready');

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------
function initNav() {
    var navItems = document.querySelectorAll('.nav-item');
    navItems.forEach(function(item) {
        item.addEventListener('click', function() {
            var viewId = this.getAttribute('data-view');
            showView(viewId);
            navItems.forEach(function(n) { n.classList.remove('active'); });
            this.classList.add('active');
        });
    });
}

function showView(id) {
    var views = document.querySelectorAll('.view');
    views.forEach(function(v) { v.classList.remove('active'); });
    var target = document.getElementById(id);
    if (target) { target.classList.add('active'); }
}

// ---------------------------------------------------------------------------
// State management
// ---------------------------------------------------------------------------
var _pollInterval = null;
var _lastOutputLen = 0;

function loadState() {
    window.getState().then(function(state) {
        console.log('State loaded:', state);
    }).catch(function(e) {
        console.warn('Could not load state:', e);
    });
}

function startOutputPolling(outputEl) {
    stopOutputPolling();
    _lastOutputLen = 0;
    _pollInterval = setInterval(function() {
        window.getOutput().then(function(resp) {
            if (resp && resp.output !== undefined) {
                var newOutput = resp.output;
                if (newOutput.length !== _lastOutputLen) {
                    outputEl.textContent = newOutput;
                    outputEl.scrollTop = outputEl.scrollHeight;
                    _lastOutputLen = newOutput.length;
                }
            }
        }).catch(function(e) { console.warn('getOutput poll error:', e); });
    }, 500);
}

function stopOutputPolling() {
    if (_pollInterval) {
        clearInterval(_pollInterval);
        _pollInterval = null;
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
function initConfig() {
    var btn = document.getElementById('btn-save-config');
    btn.addEventListener('click', function() {
        var apiKey = document.getElementById('cfg-api-key').value.trim();
        if (!apiKey) { alert('API key is required'); return; }
        var config = {
            model: document.getElementById('cfg-model').value,
            baseUrl: document.getElementById('cfg-base-url').value,
            apiKey: apiKey,
            cdpPort: parseInt(document.getElementById('cfg-cdp-port').value, 10)
        };
        window.setConfig(config).then(function(resp) {
            alert('Config saved!');
        }).catch(function(e) {
            alert('Error saving config: ' + e);
        });
    });
}

// ---------------------------------------------------------------------------
// Explore
// ---------------------------------------------------------------------------
function initExplore() {
    var btn = document.getElementById('btn-explore');
    var output = document.getElementById('explore-output');
    btn.addEventListener('click', function() {
        var url = document.getElementById('explore-url').value;
        var depth = parseInt(document.getElementById('explore-depth').value, 10) || 3;
        if (!url) { alert('Please enter a URL'); return; }
        btn.disabled = true;
        output.textContent = 'Starting explore...\n';
        startOutputPolling(output);
        window.runExplore(url, depth).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
            stopOutputPolling();
        });
    });
}

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------
function initGenerate() {
    var btn = document.getElementById('btn-generate');
    var output = document.getElementById('generate-output');
    btn.addEventListener('click', function() {
        var requirement = document.getElementById('gen-requirement').value;
        var spec = document.getElementById('gen-spec').value;
        if (!requirement) { alert('Please enter a requirement'); return; }
        btn.disabled = true;
        output.textContent = 'Generating tests...\n';
        startOutputPolling(output);
        window.runGenerate(requirement, spec).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
            stopOutputPolling();
        });
    });
}

// ---------------------------------------------------------------------------
// Run Tests
// ---------------------------------------------------------------------------
function initRun() {
    var btn = document.getElementById('btn-run');
    var output = document.getElementById('run-output');
    var projectSelect = document.getElementById('run-project');

    // Load projects
    window.getProjects().then(function(projects) {
        projectSelect.innerHTML = '<option value="">Select project...</option>';
        projects.forEach(function(p) {
            var opt = document.createElement('option');
            opt.value = p;
            opt.textContent = p;
            projectSelect.appendChild(opt);
        });
    }).catch(function(e) {
        console.warn('Could not load projects:', e);
    });

    btn.addEventListener('click', function() {
        var project = projectSelect.value;
        var type = document.getElementById('run-type').value;
        if (!project) { alert('Please select a project'); return; }
        btn.disabled = true;
        output.textContent = 'Running tests...\n';
        startOutputPolling(output);
        window.runTests(project, type).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
            stopOutputPolling();
        });
    });
}

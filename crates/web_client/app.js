// QinAegis Web Client JS
// Provides UI logic and bridges to Rust via RPC

document.addEventListener('DOMContentLoaded', function() {
    initNav();
    initConfig();
    initExplore();
    initGenerate();
    initRun();
    loadState();
});

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
function loadState() {
    window.getState().then(function(state) {
        console.log('State loaded:', state);
    }).catch(function(e) {
        console.warn('Could not load state:', e);
    });
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
function initConfig() {
    var btn = document.getElementById('btn-save-config');
    btn.addEventListener('click', function() {
        var config = {
            model: document.getElementById('cfg-model').value,
            baseUrl: document.getElementById('cfg-base-url').value,
            apiKey: document.getElementById('cfg-api-key').value,
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
        window.runExplore(url, depth).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
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
        window.runGenerate(requirement, spec).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
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
        window.runTests(project, type).then(function(resp) {
            output.textContent += 'Job started: ' + JSON.stringify(resp) + '\n';
            btn.disabled = false;
        }).catch(function(e) {
            output.textContent += 'Error: ' + e + '\n';
            btn.disabled = false;
        });
    });
}

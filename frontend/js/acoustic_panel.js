(function (global) {
    var API_BASE = window.location.protocol === 'file:' ? 'http://localhost:8080' : '';
    var currentSite = 'huiyinbi';
    var pollIntervalId = null;

    function getApiBase() {
        return API_BASE;
    }

    async function apiCall(method, path, body) {
        try {
            document.getElementById('status-dot').classList.remove('disconnected');
            document.getElementById('status-text').textContent = '通信中...';
            var opts = { method: method, headers: { 'Content-Type': 'application/json' } };
            if (body) opts.body = JSON.stringify(body);
            var res = await fetch(API_BASE + path, opts);
            document.getElementById('status-text').textContent = '后端已连接';
            if (!res.ok) throw new Error('HTTP ' + res.status);
            return await res.json();
        } catch (e) {
            document.getElementById('status-dot').classList.add('disconnected');
            document.getElementById('status-text').textContent = '连接失败: ' + e.message;
            return null;
        }
    }

    async function runAcousticSimulation() {
        var t3d = global.TempleOfHeaven3D;
        var site = t3d.getCurrentSite();
        var params = t3d.getSimParams();
        var sitePos = {
            huiyinbi: { x: 0, y: 1.5, z: 0 },
            sanyinshi: { x: 0, y: 0.5, z: 5 },
            huanqiutan: { x: 0, y: 5.8, z: -30 }
        }[site];
        var result = await apiCall('POST', '/api/simulate/acoustics', {
            site_id: site,
            source_position: sitePos,
            frequency: params.frequency,
            max_reflections: params.reflections,
            num_rays: params.rays,
            temperature: 20,
            humidity: 50
        });
        if (result && result.success && result.data) {
            t3d.spawnParticlesFromPaths(result.data);
            if (result.data.length > 0) {
                var avgAtt = result.data.reduce(function (s, p) { return s + p.attenuation_db; }, 0) / result.data.length;
                var avgT = result.data.reduce(function (s, p) { return s + p.travel_time; }, 0) / result.data.length;
                var t60 = Math.max(0.5, avgT * 3);
                document.getElementById('val-t60').innerHTML = t60.toFixed(2) + '<span class="metric-unit">s</span>';
                document.getElementById('val-edt').innerHTML = (t60 * 0.8).toFixed(2) + '<span class="metric-unit">s</span>';
                document.getElementById('val-spl').innerHTML = (85 - avgAtt * 0.3).toFixed(1) + '<span class="metric-unit">dB</span>';
            }
        }
    }

    async function runWaveSimulation() {
        var t3d = global.TempleOfHeaven3D;
        var site = t3d.getCurrentSite();
        var params = t3d.getSimParams();
        var sitePos = {
            huiyinbi: { x: 0, y: 1.5, z: 0 },
            sanyinshi: { x: 0, y: 0.5, z: 5 },
            huanqiutan: { x: 0, y: 5.8, z: -30 }
        }[site];
        var result = await apiCall('POST', '/api/simulate/wave-field', {
            site_id: site,
            source_position: sitePos,
            frequency: params.frequency,
            max_reflections: params.reflections,
            num_rays: params.rays,
            temperature: 20,
            humidity: 50
        });
        if (result && result.success && result.data) {
            t3d.drawSoundField(result.data);
        }
    }

    async function runStiAnalysis() {
        var ir_len = 512;
        var ir = [];
        for (var i = 0; i < ir_len; i++) {
            var t = i / 44100;
            var decay = Math.exp(-3 * t);
            var peak = i === 42 ? 1.0 : 0;
            var echo1 = Math.abs(i - 180) < 10 ? 0.5 * Math.exp(-Math.pow(i - 180, 2) / 100) : 0;
            var echo2 = Math.abs(i - 320) < 15 ? 0.35 * Math.exp(-Math.pow(i - 320, 2) / 200) : 0;
            ir.push((peak + echo1 + echo2) * decay + (Math.random() - 0.5) * 0.02);
        }
        var site = global.TempleOfHeaven3D.getCurrentSite();
        var result = await apiCall('POST', '/api/simulate/sti', {
            site_id: site,
            impulse_response: ir,
            sample_rate: 44100,
            background_noise_level: 35,
            speech_level: 70
        });
        if (result && result.success && result.data) {
            var d = result.data;
            document.getElementById('val-sti').textContent = d.sti_value.toFixed(3);
            document.getElementById('val-rasti').textContent = d.rasti_value.toFixed(3);
            document.getElementById('val-c50').innerHTML = d.clarity_c50.toFixed(1) + '<span class="metric-unit">dB</span>';
            var stiPct = Math.max(0, Math.min(100, d.sti_value * 100));
            document.getElementById('sti-bar').style.width = stiPct + '%';
            var q;
            if (d.sti_value >= 0.85) q = '优秀 · 祭祀语音清晰可辨';
            else if (d.sti_value >= 0.75) q = '良好 · 语音信息传达完整';
            else if (d.sti_value >= 0.60) q = '中等 · 主要语义可理解';
            else if (d.sti_value >= 0.45) q = '较差 · 需仔细聆听';
            else if (d.sti_value >= 0.30) q = '差 · 部分语音失真';
            else q = '不可接受 · 声学特性严重退化';
            document.getElementById('sti-quality').textContent = q;
        }
    }

    async function loadLatestData(site) {
        currentSite = site;
        var results = await Promise.all([
            apiCall('GET', '/api/measurements/' + site + '?limit=1'),
            apiCall('GET', '/api/intelligibility?site_id=' + site + '&limit=1'),
            apiCall('GET', '/api/alerts?limit=10')
        ]);
        var measRes = results[0];
        var stiRes = results[1];
        var alertRes = results[2];
        if (measRes && measRes.success && measRes.data && measRes.data.length > 0) {
            var m = measRes.data[0];
            document.getElementById('val-t60').innerHTML = m.reverb_time_t60.toFixed(2) + '<span class="metric-unit">s</span>';
            document.getElementById('val-edt').innerHTML = m.reverb_time_edt.toFixed(2) + '<span class="metric-unit">s</span>';
            document.getElementById('val-spl').innerHTML = m.sound_pressure_level.toFixed(1) + '<span class="metric-unit">dB</span>';
        }
        if (stiRes && stiRes.success && stiRes.data && stiRes.data.length > 0) {
            var s = stiRes.data[0];
            document.getElementById('val-sti').textContent = s.sti_value.toFixed(3);
            document.getElementById('val-rasti').textContent = s.rasti_value.toFixed(3);
            document.getElementById('val-c50').innerHTML = s.clarity_c50.toFixed(1) + '<span class="metric-unit">dB</span>';
            document.getElementById('sti-bar').style.width = Math.max(0, Math.min(100, s.sti_value * 100)) + '%';
        }
        renderAlerts(alertRes);
    }

    function renderAlerts(res) {
        var container = document.getElementById('alert-list');
        if (!res || !res.success || !res.data || res.data.length === 0) {
            container.innerHTML = '<div style="color:#808070;font-size:11px;text-align:center;padding:10px;">暂无告警信息</div>';
            return;
        }
        container.innerHTML = res.data.map(function (a) {
            var sev = a.severity || 'info';
            var cls = sev === 'critical' ? '' : sev === 'warning' ? 'warning' : 'info';
            var t = new Date(a.timestamp || Date.now());
            return '<div class="alert-item ' + cls + '">' +
                '<span class="alert-severity ' + sev + '">' + sev.toUpperCase() + '</span>' +
                '<span style="color:#a08050;margin-left:6px;">' + a.metric_name + '</span>' +
                '<div class="alert-desc">' + a.description + '</div>' +
                '<div class="alert-time">' + t.toLocaleTimeString() + '</div>' +
                '</div>';
        }).join('');
    }

    function updateParam(key, value) {
        var paramKey = key === 'freq' ? 'frequency' : key === 'refl' ? 'reflections' : key === 'rays' ? 'rays' : 'speed';
        var update = {};
        update[paramKey] = Number(value);
        global.TempleOfHeaven3D.setSimParams(update);
        document.getElementById(key + '-value').textContent = value;
    }

    function init() {
        currentSite = 'huiyinbi';
        setTimeout(function () { loadLatestData(currentSite); }, 800);
        if (pollIntervalId) clearInterval(pollIntervalId);
        pollIntervalId = setInterval(function () { loadLatestData(currentSite); }, 10000);
    }

    global.AcousticPanel = {
        init: init,
        apiCall: apiCall,
        runAcousticSimulation: runAcousticSimulation,
        runWaveSimulation: runWaveSimulation,
        runStiAnalysis: runStiAnalysis,
        loadLatestData: loadLatestData,
        renderAlerts: renderAlerts,
        updateParam: updateParam,
        getApiBase: getApiBase
    };
})(window);

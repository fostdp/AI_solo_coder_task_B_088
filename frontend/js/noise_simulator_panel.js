const NoiseSimulatorPanel = (function() {
    const API_BASE = SharedUtils.API_BASE;
    let noiseMarkers = [];

    function init() {
    }

    async function runNoiseSimulation() {
        const resultSection = document.getElementById('noise-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const visitorCount = parseInt(document.getElementById('noise-visitor-count')?.value || 100);
        const sourceLevel = parseFloat(document.getElementById('noise-source-level')?.value || 60);
        const distribution = document.getElementById('noise-distribution')?.value || 'uniform';

        const btnState = SharedUtils.setButtonLoading(event.target, '⏳ 模拟中...');

        const noiseSources = generateNoiseSources(visitorCount, sourceLevel, distribution);

        try {
            const response = await fetch(`${API_BASE}/simulate/noise`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_id: 'huiyinbi',
                    source_position: { x: 0, y: 1.6, z: 20 },
                    listener_position: { x: 10, y: 1.6, z: 0 },
                    noise_sources: noiseSources,
                    speech_level_db: 70,
                    frequency: 1000
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayNoiseResult(data.data);
                showNoiseMarkers(noiseSources);
            } else {
                if (resultSection) {
                    resultSection.style.display = 'block';
                    resultSection.innerHTML = `<div class="error">模拟失败: ${data.message}</div>`;
                }
            }
        } catch (e) {
            if (resultSection) {
                resultSection.style.display = 'block';
                resultSection.innerHTML = `<div class="error">网络错误: ${e.message}</div>`;
            }
        } finally {
            SharedUtils.resetButton(btnState, '🔊 运行噪声模拟');
        }
    }

    function generateNoiseSources(count, level, distribution) {
        const sources = [];
        const displayCount = Math.min(count, 20);

        for (let i = 0; i < displayCount; i++) {
            let x, z;
            if (distribution === 'uniform') {
                const angle = Math.random() * Math.PI * 2;
                const radius = 5 + Math.random() * 25;
                x = Math.cos(angle) * radius;
                z = Math.sin(angle) * radius;
            } else if (distribution === 'cluster') {
                const clusterX = [15, -15, 0][i % 3];
                const clusterZ = [0, 10, -10][i % 3];
                x = clusterX + (Math.random() - 0.5) * 8;
                z = clusterZ + (Math.random() - 0.5) * 8;
            } else {
                const angle = (i / displayCount) * Math.PI * 2;
                const radius = 15 + (Math.random() - 0.5) * 3;
                x = Math.cos(angle) * radius;
                z = Math.sin(angle) * radius;
            }

            sources.push({
                position: { x, y: 1.6, z },
                sound_level_db: level + (Math.random() - 0.5) * 10,
                source_type: `visitor_${i}`,
                frequency_hz: 500
            });
        }
        return sources;
    }

    function showNoiseMarkers(sources) {
        if (typeof Buildings3D === 'undefined') return;
        const scene = Buildings3D.getScene();
        if (!scene) return;

        hideAllNoiseMarkers();

        sources.forEach(src => {
            const marker = Buildings3D.createNoiseMarker(
                { x: src.position.x, z: src.position.z },
                src.sound_level_db
            );
            scene.add(marker);
            noiseMarkers.push(marker);
        });
    }

    function hideAllNoiseMarkers() {
        const scene = typeof Buildings3D !== 'undefined' ? Buildings3D.getScene() : null;
        if (!scene) return;

        noiseMarkers.forEach(m => {
            scene.remove(m);
        });
        noiseMarkers = [];
    }

    function displayNoiseResult(result) {
        const section = document.getElementById('noise-result-section');
        if (!section) return;
        section.style.display = 'block';

        const leqEl = document.getElementById('noise-leq');
        if (leqEl) leqEl.textContent = result.total_noise_level_db.toFixed(1) + ' dB';

        const snrEl = document.getElementById('noise-snr');
        if (snrEl) {
            snrEl.textContent = result.snr_db.toFixed(1) + ' dB';
            snrEl.className = 'value ' + (result.snr_db < 0 ? 'danger' : result.snr_db < 10 ? 'warning' : 'success');
        }

        const cleanSti = result.sti_clean || 0.75;
        const noisySti = result.sti_noisy || 0.4;
        const degradation = result.sti_degradation || (cleanSti - noisySti);

        const cleanBar = document.getElementById('noise-clean-sti-bar');
        const cleanText = document.getElementById('noise-clean-sti-text');
        if (cleanBar) cleanBar.style.width = (cleanSti * 100) + '%';
        if (cleanText) cleanText.textContent = (cleanSti * 100).toFixed(1) + '%';

        const noisyBar = document.getElementById('noise-noisy-sti-bar');
        const noisyText = document.getElementById('noise-noisy-sti-text');
        if (noisyBar) noisyBar.style.width = (noisySti * 100) + '%';
        if (noisyText) noisyText.textContent = (noisySti * 100).toFixed(1) + '%';

        const degEl = document.getElementById('noise-sti-degradation');
        if (degEl) degEl.textContent = '-' + (degradation * 100).toFixed(1) + '%';
    }

    return {
        init,
        runNoiseSimulation,
        hideAllNoiseMarkers
    };
})();

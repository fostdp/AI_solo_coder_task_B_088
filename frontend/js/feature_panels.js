/**
 * 新功能面板模块：朝代对比、古今对比、噪声模拟、虚拟体验
 * 依赖: THREE.js, Buildings3D, TempleOfHeaven3D
 */

const FeaturePanels = (function() {
    const API_BASE = '/api';
    let speakerMarker = null;
    let listenerMarker = null;
    let noiseMarkers = [];
    let ancientBuildingsCache = null;
    let concertHallsCache = null;

    function init() {
    }

    async function loadAncientBuildings() {
        const container = document.getElementById('dynasty-building-list');
        if (!container) return;

        if (ancientBuildingsCache) {
            renderBuildingList(container, ancientBuildingsCache);
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/buildings/ancient`);
            const data = await response.json();
            if (data.success && data.data) {
                ancientBuildingsCache = data.data;
                renderBuildingList(container, data.data);
            } else {
                container.innerHTML = '<div class="error">加载建筑列表失败</div>';
            }
        } catch (e) {
            container.innerHTML = '<div class="error">网络错误，使用本地数据</div>';
            ancientBuildingsCache = getDefaultAncientBuildings();
            renderBuildingList(container, ancientBuildingsCache);
        }
    }

    async function loadConcertHalls() {
        const container = document.getElementById('modern-hall-list');
        if (!container) return;

        if (concertHallsCache) {
            renderBuildingList(container, concertHallsCache);
            return;
        }

        try {
            const response = await fetch(`${API_BASE}/buildings/concert-halls`);
            const data = await response.json();
            if (data.success && data.data) {
                concertHallsCache = data.data;
                renderBuildingList(container, data.data);
            } else {
                container.innerHTML = '<div class="error">加载音乐厅列表失败</div>';
            }
        } catch (e) {
            container.innerHTML = '<div class="error">网络错误，使用本地数据</div>';
            concertHallsCache = getDefaultConcertHalls();
            renderBuildingList(container, concertHallsCache);
        }
    }

    function getDefaultAncientBuildings() {
        return [
            { building_id: 'tang_temple', name: '唐代明堂', dynasty: '唐代', typical_t60: 3.8 },
            { building_id: 'song_temple', name: '宋代大庆殿', dynasty: '宋代', typical_t60: 2.8 },
            { building_id: 'ming_temple', name: '明代奉天殿', dynasty: '明代', typical_t60: 3.2 },
            { building_id: 'qing_temple', name: '清代太和殿', dynasty: '清代', typical_t60: 2.5 }
        ];
    }

    function getDefaultConcertHalls() {
        return [
            { building_id: 'shoemaker_hall', name: '鞋盒式音乐厅', architecture_style: 'Shoebox', typical_t60: 2.0 },
            { building_id: 'vineyard_hall', name: '葡萄园式音乐厅', architecture_style: 'Vineyard', typical_t60: 1.8 },
            { building_id: 'boston_hall', name: '波士顿交响乐厅', architecture_style: 'Classical', typical_t60: 1.9 }
        ];
    }

    function renderBuildingList(container, buildings) {
        let html = '<div style="display: flex; flex-wrap: wrap; gap: 6px;">';
        buildings.forEach(b => {
            const label = b.dynasty ? `${b.dynasty}` : (b.architecture_style || '');
            html += `
                <div style="flex: 1; min-width: 120px; padding: 8px; background: rgba(20, 30, 50, 0.6); border: 1px solid rgba(212, 175, 55, 0.3); border-radius: 6px;">
                    <div style="font-size: 12px; color: #d4af37; font-weight: bold;">${b.name}</div>
                    <div style="font-size: 10px; color: #a08050; margin-top: 4px;">${label} · T60: ${b.typical_t60}s</div>
                </div>
            `;
        });
        html += '</div>';
        container.innerHTML = html;
    }

    async function runDynastyComparison() {
        const resultSection = document.getElementById('dynasty-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const btn = event.target;
        if (btn) {
            const originalText = btn.textContent;
            btn.textContent = '⏳ 计算中...';
            btn.disabled = true;
        }

        try {
            const siteIds = ['tang_temple', 'song_temple', 'ming_temple', 'qing_temple', 'huiyinbi'];
            const response = await fetch(`${API_BASE}/compare/acoustics`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_ids: siteIds,
                    frequency: 1000,
                    background_noise_db: 35
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayDynastyComparison(data.data);
            } else {
                showComparisonError('dynasty-result-section', data.message || '对比分析失败');
            }
        } catch (e) {
            showComparisonError('dynasty-result-section', '网络错误: ' + e.message);
        } finally {
            if (btn) {
                btn.textContent = '📊 开始朝代对比分析';
                btn.disabled = false;
            }
        }
    }

    async function runModernComparison() {
        const resultSection = document.getElementById('modern-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const btn = event.target;
        if (btn) {
            btn.textContent = '⏳ 计算中...';
            btn.disabled = true;
        }

        try {
            const siteIds = ['huiyinbi', 'ming_temple', 'qing_temple', 'shoemaker_hall', 'vineyard_hall', 'boston_hall'];
            const response = await fetch(`${API_BASE}/compare/acoustics`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_ids: siteIds,
                    frequency: 1000,
                    background_noise_db: 30
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayModernComparison(data.data);
            } else {
                showComparisonError('modern-result-section', data.message || '对比分析失败');
            }
        } catch (e) {
            showComparisonError('modern-result-section', '网络错误: ' + e.message);
        } finally {
            if (btn) {
                btn.textContent = '🎵 开始古今对比分析';
                btn.disabled = false;
            }
        }
    }

    function showComparisonError(sectionId, message) {
        const section = document.getElementById(sectionId);
        if (section) {
            section.style.display = 'block';
            section.innerHTML = `<div class="error">${message}</div>`;
        }
    }

    function displayDynastyComparison(result) {
        const section = document.getElementById('dynasty-result-section');
        if (!section) return;
        section.style.display = 'block';

        displayBestSummary('dynasty-best-summary', result);
        displayComparisonTable('dynasty-comparison-table', result);
        displayRanking('dynasty-ranking', result);
    }

    function displayModernComparison(result) {
        const section = document.getElementById('modern-result-section');
        if (!section) return;
        section.style.display = 'block';

        displayBestSummary('modern-best-summary', result);
        displayComparisonTable('modern-comparison-table', result);
        displayRanking('modern-ranking', result);
    }

    function displayBestSummary(containerId, result) {
        const container = document.getElementById(containerId);
        if (!container) return;

        container.innerHTML = `
            <div class="best-item">
                <span class="best-label">🏆 最佳语音</span>
                <span class="best-value">${getSiteName(result.best_for_speech)}</span>
            </div>
            <div class="best-item">
                <span class="best-label">🎵 最佳音乐</span>
                <span class="best-value">${getSiteName(result.best_for_music)}</span>
            </div>
            <div class="best-item">
                <span class="best-label">🔊 最强回声</span>
                <span class="best-value">${getSiteName(result.best_for_echo)}</span>
            </div>
        `;
    }

    function displayComparisonTable(tableId, result) {
        const table = document.getElementById(tableId);
        if (!table) return;

        const sites = result.sites;
        const metrics = result.comparison_metrics || [];

        const displayMetrics = metrics.filter(m =>
            ['reverb_time_t60', 'clarity_c50', 'definition_d50', 'sti_value',
             'center_time', 'bass_ratio', 'intimacy', 'warmth', 'echo_strength'].includes(m.metric_name)
        );

        let theadHtml = '<tr><th>参数</th>';
        sites.forEach(s => {
            theadHtml += `<th>${s.site_name || s.name || s.site_id}</th>`;
        });
        theadHtml += '</tr>';
        table.querySelector('thead').innerHTML = theadHtml;

        let tbodyHtml = '';
        displayMetrics.forEach(metric => {
            tbodyHtml += `<tr><td class="metric-name">${metric.description}</td>`;
            sites.forEach(site => {
                const siteId = site.site_id || site.id;
                const value = metric.values[siteId];
                const isBest = metric.best_site === siteId;
                const displayValue = formatMetricValue(metric.metric_name, value);
                tbodyHtml += `<td class="${isBest ? 'best-value' : ''}">${isBest ? '⭐ ' : ''}${displayValue}</td>`;
            });
            tbodyHtml += '</tr>';
        });
        table.querySelector('tbody').innerHTML = tbodyHtml;
    }

    function displayRanking(containerId, result) {
        const container = document.getElementById(containerId);
        if (!container) return;

        const ranking = result.overall_ranking || [];
        let html = '';
        ranking.forEach((id, idx) => {
            html += `
                <div class="ranking-item">
                    <span class="rank-number">${idx + 1}</span>
                    <span class="rank-name">${getSiteName(id)}</span>
                </div>
            `;
        });
        container.innerHTML = html;
    }

    async function runNoiseSimulation() {
        const resultSection = document.getElementById('noise-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const visitorCount = parseInt(document.getElementById('noise-visitor-count')?.value || 100);
        const sourceLevel = parseFloat(document.getElementById('noise-source-level')?.value || 60);
        const distribution = document.getElementById('noise-distribution')?.value || 'uniform';

        const btn = event.target;
        if (btn) {
            btn.textContent = '⏳ 模拟中...';
            btn.disabled = true;
        }

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
            if (btn) {
                btn.textContent = '🔊 运行噪声模拟';
                btn.disabled = false;
            }
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

    async function runVirtualExperience() {
        const resultSection = document.getElementById('experience-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const site = document.getElementById('experience-site')?.value || 'huiyinbi';
        const srcPos = document.getElementById('experience-source-pos')?.value || 'east';
        const lstPos = document.getElementById('experience-listener-pos')?.value || 'west';

        const sourcePos = positionNameToCoord(srcPos, true);
        const listenerPos = positionNameToCoord(lstPos, false);

        const btn = event.target;
        if (btn) {
            btn.textContent = '⏳ 计算中...';
            btn.disabled = true;
        }

        showExperienceMarkers(sourcePos, listenerPos);

        try {
            const response = await fetch(`${API_BASE}/experience/virtual`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_id: site,
                    source_position: { x: sourcePos.x, y: 1.6, z: sourcePos.z },
                    listener_position: { x: listenerPos.x, y: 1.6, z: listenerPos.z },
                    frequency: 1000,
                    include_noise: false
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayExperienceResult(data.data, srcPos, lstPos);
            } else {
                if (resultSection) {
                    resultSection.style.display = 'block';
                    resultSection.innerHTML = `<div class="error">计算失败: ${data.message}</div>`;
                }
            }
        } catch (e) {
            if (resultSection) {
                resultSection.style.display = 'block';
                resultSection.innerHTML = `<div class="error">网络错误: ${e.message}</div>`;
            }
        } finally {
            if (btn) {
                btn.textContent = '🎧 开始虚拟体验';
                btn.disabled = false;
            }
        }
    }

    function positionNameToCoord(name, isSource) {
        const radius = 15;
        switch (name) {
            case 'east': return { x: radius, z: 0 };
            case 'west': return { x: -radius, z: 0 };
            case 'north': return { x: 0, z: -radius };
            case 'south': return { x: 0, z: radius };
            case 'center': return { x: 0, z: 0 };
            default: return { x: radius, z: 0 };
        }
    }

    function showExperienceMarkers(sourcePos, listenerPos) {
        if (typeof Buildings3D === 'undefined') return;
        const scene = Buildings3D.getScene();
        if (!scene) return;

        if (speakerMarker) {
            scene.remove(speakerMarker);
        }
        if (listenerMarker) {
            scene.remove(listenerMarker);
        }

        speakerMarker = Buildings3D.createSpeakerMarker(
            { x: sourcePos.x, z: sourcePos.z },
            true
        );
        scene.add(speakerMarker);

        listenerMarker = Buildings3D.createListenerMarker(
            { x: listenerPos.x, z: listenerPos.z }
        );
        scene.add(listenerMarker);
    }

    function displayExperienceResult(result, srcPosName, lstPosName) {
        const section = document.getElementById('experience-result-section');
        if (!section) return;
        section.style.display = 'block';

        const directSpl = result.direct_sound_level || 65;
        const reflectSpl = result.reflection_level || 58;
        const t60 = result.reverberation_time_t60 || 2.5;
        const sti = result.sti_without_noise || 0.7;

        const directEl = document.getElementById('exp-direct-spl');
        if (directEl) directEl.textContent = directSpl.toFixed(1) + ' dB';

        const reflectEl = document.getElementById('exp-reflect-spl');
        if (reflectEl) reflectEl.textContent = reflectSpl.toFixed(1) + ' dB';

        const t60El = document.getElementById('exp-t60');
        if (t60El) t60El.textContent = t60.toFixed(2) + ' s';

        const stiEl = document.getElementById('exp-sti');
        if (stiEl) stiEl.textContent = (sti * 100).toFixed(1) + '%';

        const binaural = result.binaural_ir || { itd_seconds: 0.0006, ild_db: 2.5 };
        const itdEl = document.getElementById('exp-itd');
        if (itdEl) itdEl.textContent = (binaural.itd_seconds * 1000).toFixed(2) + ' ms';

        const ildEl = document.getElementById('exp-ild');
        if (ildEl) ildEl.textContent = binaural.ild_db.toFixed(2) + ' dB';

        const descEl = document.getElementById('exp-description');
        if (descEl) {
            descEl.textContent = generateExperienceDescription(result, srcPosName, lstPosName);
        }
    }

    function generateExperienceDescription(result, srcPos, lstPos) {
        const positions = { east: '东侧', west: '西侧', north: '北侧', south: '南侧', center: '中心' };
        const srcName = positions[srcPos] || srcPos;
        const lstName = positions[lstPos] || lstPos;

        const sti = result.sti_without_noise || 0.7;
        const quality = sti >= 0.75 ? '清晰可辨' : sti >= 0.6 ? '较为清晰' : '略显模糊';

        if (srcPos === lstPos) {
            return `您站在${srcName}说话，可以听到明显的回音效果。声波沿圆形墙壁反射后回到原点，形成独特的自回音现象。语音清晰度：${quality}。建议戴上立体声耳机体验双耳听觉效果。`;
        } else {
            return `您从${srcName}说话，在${lstName}收听。声音沿回音壁弧形墙面传播，绕过障碍物到达对侧。这就是天坛回音壁"隔墙有耳"的奇妙现象！语音清晰度：${quality}。建议戴上立体声耳机体验双耳听觉效果。`;
        }
    }

    function getSiteName(siteId) {
        const names = {
            huiyinbi: '回音壁',
            sanyinshi: '三音石',
            huanqiutan: '圜丘坛',
            tang_temple: '唐代明堂',
            song_temple: '宋代大庆殿',
            ming_temple: '明代奉天殿',
            qing_temple: '清代太和殿',
            shoemaker_hall: '鞋盒式音乐厅',
            vineyard_hall: '葡萄园式音乐厅',
            boston_hall: '波士顿交响乐厅'
        };
        return names[siteId] || siteId;
    }

    function formatMetricValue(metricName, value) {
        if (value === undefined || value === null) return '-';
        switch (metricName) {
            case 'reverb_time_t60':
            case 'reverb_time_edt':
            case 'center_time':
                return value.toFixed(2) + ' s';
            case 'clarity_c50':
            case 'sound_pressure_level':
                return value.toFixed(1) + ' dB';
            case 'definition_d50':
                return value.toFixed(1) + '%';
            case 'sti_value':
            case 'rasti_value':
            case 'intimacy':
            case 'warmth':
            case 'loudness':
            case 'brilliance':
            case 'echo_strength':
                return (value * 100).toFixed(1) + '%';
            case 'bass_ratio':
                return value.toFixed(2);
            default:
                return value.toFixed(2);
        }
    }

    function getStiQuality(sti) {
        if (sti >= 0.85) return { label: '优秀', color: '#00ff88' };
        if (sti >= 0.75) return { label: '良好', color: '#44cc44' };
        if (sti >= 0.60) return { label: '中等', color: '#ffaa00' };
        if (sti >= 0.45) return { label: '较差', color: '#ff6600' };
        if (sti >= 0.30) return { label: '差', color: '#ff3333' };
        return { label: '不可接受', color: '#aa0000' };
    }

    return {
        init,
        loadAncientBuildings,
        loadConcertHalls,
        runDynastyComparison,
        runModernComparison,
        runNoiseSimulation,
        runVirtualExperience
    };
})();

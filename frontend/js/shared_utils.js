const SharedUtils = (function() {
    const API_BASE = '/api';

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

    function showComparisonError(sectionId, message) {
        const section = document.getElementById(sectionId);
        if (section) {
            section.style.display = 'block';
            section.innerHTML = `<div class="error">${message}</div>`;
        }
    }

    function setButtonLoading(btn, loadingText) {
        if (!btn) return {};
        const originalText = btn.textContent;
        btn.textContent = loadingText;
        btn.disabled = true;
        return { btn, originalText };
    }

    function resetButton(state, originalText) {
        if (state.btn) {
            state.btn.textContent = originalText;
            state.btn.disabled = false;
        }
    }

    return {
        API_BASE,
        getSiteName,
        formatMetricValue,
        getStiQuality,
        displayBestSummary,
        displayComparisonTable,
        displayRanking,
        showComparisonError,
        setButtonLoading,
        resetButton,
    };
})();

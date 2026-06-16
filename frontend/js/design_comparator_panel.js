const DesignComparatorPanel = (function() {
    const API_BASE = SharedUtils.API_BASE;
    let ancientBuildingsCache = null;

    function init() {
        loadAncientBuildings();
    }

    function getDefaultAncientBuildings() {
        return [
            { building_id: 'tang_temple', name: '唐代明堂', dynasty: '唐代', typical_t60: 3.8 },
            { building_id: 'song_temple', name: '宋代大庆殿', dynasty: '宋代', typical_t60: 2.8 },
            { building_id: 'ming_temple', name: '明代奉天殿', dynasty: '明代', typical_t60: 3.2 },
            { building_id: 'qing_temple', name: '清代太和殿', dynasty: '清代', typical_t60: 2.5 }
        ];
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

    function renderBuildingList(container, buildings) {
        let html = '<div style="display: flex; flex-wrap: wrap; gap: 6px;">';
        buildings.forEach(b => {
            const label = b.dynasty ? `${b.dynasty}` : '';
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

        const btnState = SharedUtils.setButtonLoading(event.target, '⏳ 计算中...');

        try {
            const siteIds = ['tang_temple', 'song_temple', 'ming_temple', 'qing_temple', 'huiyinbi'];
            const response = await fetch(`${API_BASE}/compare/acoustics`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_ids: siteIds,
                    frequency: 1000,
                    background_noise_db: 35,
                    comparison_type: 'design'
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayDynastyComparison(data.data);
            } else {
                SharedUtils.showComparisonError('dynasty-result-section', data.message || '对比分析失败');
            }
        } catch (e) {
            SharedUtils.showComparisonError('dynasty-result-section', '网络错误: ' + e.message);
        } finally {
            SharedUtils.resetButton(btnState, '📊 开始朝代对比分析');
        }
    }

    function displayDynastyComparison(result) {
        const section = document.getElementById('dynasty-result-section');
        if (!section) return;
        section.style.display = 'block';

        SharedUtils.displayBestSummary('dynasty-best-summary', result);
        SharedUtils.displayComparisonTable('dynasty-comparison-table', result);
        SharedUtils.displayRanking('dynasty-ranking', result);
    }

    return {
        init,
        loadAncientBuildings,
        runDynastyComparison
    };
})();

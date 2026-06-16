const EraComparatorPanel = (function() {
    const API_BASE = SharedUtils.API_BASE;
    let concertHallsCache = null;

    function init() {
        loadConcertHalls();
    }

    function getDefaultConcertHalls() {
        return [
            { building_id: 'shoemaker_hall', name: '鞋盒式音乐厅', architecture_style: 'Shoebox', typical_t60: 2.0 },
            { building_id: 'vineyard_hall', name: '葡萄园式音乐厅', architecture_style: 'Vineyard', typical_t60: 1.8 },
            { building_id: 'boston_hall', name: '波士顿交响乐厅', architecture_style: 'Classical', typical_t60: 1.9 }
        ];
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

    function renderBuildingList(container, buildings) {
        let html = '<div style="display: flex; flex-wrap: wrap; gap: 6px;">';
        buildings.forEach(b => {
            const label = b.architecture_style || '';
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

    async function runModernComparison() {
        const resultSection = document.getElementById('modern-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const btnState = SharedUtils.setButtonLoading(event.target, '⏳ 计算中...');

        try {
            const siteIds = ['huiyinbi', 'ming_temple', 'qing_temple', 'shoemaker_hall', 'vineyard_hall', 'boston_hall'];
            const response = await fetch(`${API_BASE}/compare/acoustics`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    site_ids: siteIds,
                    frequency: 1000,
                    background_noise_db: 30,
                    comparison_type: 'era'
                })
            });

            const data = await response.json();
            if (data.success && data.data) {
                displayModernComparison(data.data);
            } else {
                SharedUtils.showComparisonError('modern-result-section', data.message || '对比分析失败');
            }
        } catch (e) {
            SharedUtils.showComparisonError('modern-result-section', '网络错误: ' + e.message);
        } finally {
            SharedUtils.resetButton(btnState, '🎵 开始古今对比分析');
        }
    }

    function displayModernComparison(result) {
        const section = document.getElementById('modern-result-section');
        if (!section) return;
        section.style.display = 'block';

        SharedUtils.displayBestSummary('modern-best-summary', result);
        SharedUtils.displayComparisonTable('modern-comparison-table', result);
        SharedUtils.displayRanking('modern-ranking', result);
    }

    return {
        init,
        loadConcertHalls,
        runModernComparison
    };
})();

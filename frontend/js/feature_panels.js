/**
 * 新功能面板模块（兼容包装器）
 * 
 * 注意：本文件已重构，实际实现已拆分到独立模块：
 *   - design_comparator_panel.js: 朝代对比
 *   - era_comparator_panel.js: 跨时代对比
 *   - noise_simulator_panel.js: 噪声模拟
 *   - vr_echo_wall_panel.js: 虚拟体验
 *   - shared_utils.js: 共享工具函数
 * 
 * 本文件保留原有API以保持向后兼容。
 */

const FeaturePanels = (function() {
    function init() {
        if (typeof DesignComparatorPanel !== 'undefined') DesignComparatorPanel.init();
        if (typeof EraComparatorPanel !== 'undefined') EraComparatorPanel.init();
        if (typeof NoiseSimulatorPanel !== 'undefined') NoiseSimulatorPanel.init();
        if (typeof VrEchoWallPanel !== 'undefined') VrEchoWallPanel.init();
    }

    function loadAncientBuildings() {
        if (typeof DesignComparatorPanel !== 'undefined') {
            return DesignComparatorPanel.loadAncientBuildings();
        }
    }

    function loadConcertHalls() {
        if (typeof EraComparatorPanel !== 'undefined') {
            return EraComparatorPanel.loadConcertHalls();
        }
    }

    function runDynastyComparison() {
        if (typeof DesignComparatorPanel !== 'undefined') {
            return DesignComparatorPanel.runDynastyComparison();
        }
    }

    function runModernComparison() {
        if (typeof EraComparatorPanel !== 'undefined') {
            return EraComparatorPanel.runModernComparison();
        }
    }

    function runNoiseSimulation() {
        if (typeof NoiseSimulatorPanel !== 'undefined') {
            return NoiseSimulatorPanel.runNoiseSimulation();
        }
    }

    function runVirtualExperience() {
        if (typeof VrEchoWallPanel !== 'undefined') {
            return VrEchoWallPanel.runVirtualExperience();
        }
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

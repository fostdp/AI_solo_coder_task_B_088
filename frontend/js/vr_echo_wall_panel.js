const VrEchoWallPanel = (function() {
    const API_BASE = SharedUtils.API_BASE;
    let speakerMarker = null;
    let listenerMarker = null;

    function init() {
    }

    async function runVirtualExperience() {
        const resultSection = document.getElementById('experience-result-section');
        if (resultSection) resultSection.style.display = 'none';

        const site = document.getElementById('experience-site')?.value || 'huiyinbi';
        const srcPos = document.getElementById('experience-source-pos')?.value || 'east';
        const lstPos = document.getElementById('experience-listener-pos')?.value || 'west';

        const sourcePos = positionNameToCoord(srcPos, true);
        const listenerPos = positionNameToCoord(lstPos, false);

        const btnState = SharedUtils.setButtonLoading(event.target, '⏳ 计算中...');

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
            SharedUtils.resetButton(btnState, '🎧 开始虚拟体验');
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

    return {
        init,
        runVirtualExperience
    };
})();

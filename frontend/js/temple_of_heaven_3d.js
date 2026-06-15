(function (global) {
    var MAX_PARTICLES = 4096;

    var siteColors = {
        huiyinbi: { wall: 0x8b6914, accent: 0xd4af37 },
        sanyinshi: { wall: 0x696969, accent: 0xa0a0a0 },
        huanqiutan: { wall: 0xf5f5f5, accent: 0xd4af37 }
    };

    var scene, camera, renderer, controls;
    var soundFieldCanvas, soundFieldCtx;
    var currentSite = 'huiyinbi';
    var simParams = { frequency: 1000, reflections: 8, rays: 100, speed: 5 };
    var particleRunning = true;
    var soundPaths = [];
    var lastTime = performance.now();
    var frameCount = 0;
    var particleInstances = null;
    var particleDummy = null;
    var particleData = [];
    var rayLineSegments = null;
    var rayLinePositions = null;
    var rayLineGeometry = null;
    var activeParticleCount = 0;

    function buildGround() {
        var ground = new THREE.Mesh(
            new THREE.PlaneGeometry(400, 400),
            new THREE.MeshStandardMaterial({ color: 0x1a1810, roughness: 0.95 })
        );
        ground.rotation.x = -Math.PI / 2;
        ground.receiveShadow = true;
        scene.add(ground);

        var grid = new THREE.GridHelper(300, 60, 0x4a3a1a, 0x2a2010);
        grid.position.y = 0.01;
        scene.add(grid);
    }

    function buildTiantanComplex() {
        buildHuiyinbi(0, 0, 0);
        buildSanyinshi(0, 0, 5);
        buildHuanqiutan(0, 0, -30);
    }

    function buildHuiyinbi(cx, cy, cz) {
        var radius = 30.75;
        var height = 3.72;
        var seg = 96;
        var ringGeom = new THREE.TorusGeometry(radius, 0.22, 12, seg);
        var ringMat = new THREE.MeshStandardMaterial({ color: 0x2a2010, roughness: 0.8 });
        var topRing = new THREE.Mesh(ringGeom, ringMat);
        topRing.position.set(cx, height, cz);
        topRing.rotation.x = Math.PI / 2;
        scene.add(topRing);

        var wallGeom = new THREE.CylinderGeometry(radius, radius, height, seg, 1, true);
        var wallMat = new THREE.MeshStandardMaterial({
            color: 0x8b6914, roughness: 0.85, side: THREE.DoubleSide,
            emissive: 0x2a1a05, emissiveIntensity: 0.15
        });
        var wall = new THREE.Mesh(wallGeom, wallMat);
        wall.position.set(cx, height / 2, cz);
        wall.castShadow = true;
        wall.receiveShadow = true;
        wall.userData.siteId = 'huiyinbi';
        wall.userData.isWall = true;
        scene.add(wall);

        for (var i = 0; i < 48; i++) {
            var angle = (i / 48) * Math.PI * 2;
            var x = cx + (radius - 0.1) * Math.cos(angle);
            var z = cz + (radius - 0.1) * Math.sin(angle);
            var pillar = new THREE.Mesh(
                new THREE.BoxGeometry(0.4, height, 0.6),
                new THREE.MeshStandardMaterial({ color: 0x6a5010, roughness: 0.9 })
            );
            pillar.position.set(x, height / 2, z);
            pillar.rotation.y = -angle;
            pillar.castShadow = true;
            scene.add(pillar);
        }

        var domeGeom = new THREE.SphereGeometry(4, 24, 16, 0, Math.PI * 2, 0, Math.PI / 2);
        var domeMat = new THREE.MeshStandardMaterial({
            color: 0x1a3a6a, metalness: 0.6, roughness: 0.3,
            emissive: 0x0a1a3a, emissiveIntensity: 0.2
        });
        var dome = new THREE.Mesh(domeGeom, domeMat);
        dome.position.set(cx, height + 1.5, cz);
        dome.castShadow = true;
        scene.add(dome);

        var baseGeom = new THREE.CylinderGeometry(5.5, 6, 1.2, 32);
        var base = new THREE.Mesh(baseGeom, new THREE.MeshStandardMaterial({ color: 0x9a8a6a, roughness: 0.7 }));
        base.position.set(cx, 0.6, cz);
        base.castShadow = true;
        base.receiveShadow = true;
        scene.add(base);

        var stepsGeom = new THREE.CylinderGeometry(7.5, 8, 0.3, 32);
        var steps = new THREE.Mesh(stepsGeom, new THREE.MeshStandardMaterial({ color: 0x8a7a5a, roughness: 0.8 }));
        steps.position.set(cx, 0.15, cz);
        steps.receiveShadow = true;
        scene.add(steps);

        var label = createSiteLabel("回音壁", cx, height + 8, cz, 0xd4af37);
        scene.add(label);
    }

    function buildSanyinshi(cx, cy, cz) {
        for (var i = 0; i < 3; i++) {
            var stone = new THREE.Mesh(
                new THREE.BoxGeometry(1.2, 0.2, 1.2),
                new THREE.MeshStandardMaterial({
                    color: 0x808080, roughness: 0.9,
                    emissive: 0x333300, emissiveIntensity: 0.05 + i * 0.03
                })
            );
            stone.position.set(cx, 0.1, cz + (i - 1) * 1.0);
            stone.castShadow = true;
            stone.receiveShadow = true;
            stone.userData.siteId = 'sanyinshi';
            stone.userData.stoneIndex = i + 1;
            scene.add(stone);
        }

        var plaza = new THREE.Mesh(
            new THREE.PlaneGeometry(10, 3),
            new THREE.MeshStandardMaterial({ color: 0x505050, roughness: 0.85 })
        );
        plaza.rotation.x = -Math.PI / 2;
        plaza.position.set(cx, 0.02, cz);
        plaza.receiveShadow = true;
        scene.add(plaza);

        var label = createSiteLabel("三音石", cx, 3, cz, 0xc0c0c0);
        scene.add(label);
    }

    function buildHuanqiutan(cx, cy, cz) {
        var layers = [
            { r: 11.5, y: 5.0, h: 1.6 },
            { r: 16.5, y: 3.4, h: 1.6 },
            { r: 21.5, y: 1.8, h: 1.6 }
        ];
        layers.forEach(function (layer, idx) {
            var slab = new THREE.Mesh(
                new THREE.CylinderGeometry(layer.r, layer.r + 0.3, layer.h, 64),
                new THREE.MeshStandardMaterial({
                    color: 0xf0ebe0, roughness: 0.6,
                    emissive: 0x2a2010, emissiveIntensity: 0.05
                })
            );
            slab.position.set(cx, layer.y, cz);
            slab.castShadow = true;
            slab.receiveShadow = true;
            scene.add(slab);

            var ring = new THREE.Mesh(
                new THREE.TorusGeometry(layer.r, 0.15, 8, 64),
                new THREE.MeshStandardMaterial({ color: 0xd4af37, metalness: 0.7, roughness: 0.3 })
            );
            ring.rotation.x = Math.PI / 2;
            ring.position.set(cx, layer.y + layer.h / 2, cz);
            scene.add(ring);

            for (var i = 0; i < 8; i++) {
                var angle = (i / 8) * Math.PI * 2 + idx * 0.1;
                var stair = new THREE.Mesh(
                    new THREE.BoxGeometry(1.5, layer.h + 0.2, 2),
                    new THREE.MeshStandardMaterial({ color: 0xe0d8c8, roughness: 0.7 })
                );
                stair.position.set(
                    cx + (layer.r + 1) * Math.cos(angle),
                    layer.y - 0.1,
                    cz + (layer.r + 1) * Math.sin(angle)
                );
                stair.rotation.y = -angle + Math.PI / 2;
                stair.castShadow = true;
                scene.add(stair);
            }
        });

        var centerStone = new THREE.Mesh(
            new THREE.CylinderGeometry(0.6, 0.6, 0.15, 32),
            new THREE.MeshStandardMaterial({
                color: 0xd4af37, metalness: 0.5, roughness: 0.4,
                emissive: 0xd4af37, emissiveIntensity: 0.4
            })
        );
        centerStone.position.set(cx, layers[0].y + layers[0].h / 2 + 0.08, cz);
        centerStone.userData.siteId = 'huanqiutan';
        centerStone.userData.isCenter = true;
        scene.add(centerStone);

        var glowGeom = new THREE.RingGeometry(0.7, 3.0, 32);
        var glowMat = new THREE.MeshBasicMaterial({
            color: 0xd4af37, transparent: true, opacity: 0.2, side: THREE.DoubleSide
        });
        var glow = new THREE.Mesh(glowGeom, glowMat);
        glow.rotation.x = -Math.PI / 2;
        glow.position.set(cx, layers[0].y + layers[0].h / 2 + 0.02, cz);
        scene.add(glow);

        var label = createSiteLabel("圜丘坛", cx, layers[2].y + 6, cz, 0xd4af37);
        scene.add(label);
    }

    function createSiteLabel(text, x, y, z, color) {
        var canvas = document.createElement('canvas');
        canvas.width = 256;
        canvas.height = 64;
        var ctx = canvas.getContext('2d');
        ctx.fillStyle = 'rgba(15, 20, 40, 0.9)';
        ctx.fillRect(0, 0, 256, 64);
        ctx.strokeStyle = '#' + color.toString(16).padStart(6, '0');
        ctx.lineWidth = 2;
        ctx.strokeRect(1, 1, 254, 62);
        ctx.fillStyle = '#' + color.toString(16).padStart(6, '0');
        ctx.font = 'bold 28px "Microsoft YaHei"';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(text, 128, 32);
        var tex = new THREE.CanvasTexture(canvas);
        var sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true }));
        sprite.position.set(x, y, z);
        sprite.scale.set(6, 1.5, 1);
        return sprite;
    }

    function initGPUParticles() {
        particleDummy = new THREE.Object3D();
        var geom = new THREE.SphereGeometry(0.25, 6, 4);
        var mat = new THREE.MeshBasicMaterial({
            transparent: true, depthWrite: false,
            blending: THREE.AdditiveBlending
        });
        particleInstances = new THREE.InstancedMesh(geom, mat, MAX_PARTICLES);
        particleInstances.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
        particleInstances.frustumCulled = false;
        particleInstances.count = 0;

        var colors = new Float32Array(MAX_PARTICLES * 3);
        particleInstances.instanceColor = new THREE.InstancedBufferAttribute(colors, 3);
        particleInstances.instanceColor.setUsage(THREE.DynamicDrawUsage);

        scene.add(particleInstances);

        rayLineGeometry = new THREE.BufferGeometry();
        rayLinePositions = new Float32Array(MAX_PARTICLES * 6);
        rayLineGeometry.setAttribute('position', new THREE.BufferAttribute(rayLinePositions, 3));
        rayLineGeometry.setDrawRange(0, 0);
        rayLineGeometry.attributes.position.setUsage(THREE.DynamicDrawUsage);

        var lineMat = new THREE.LineBasicMaterial({
            color: 0xd4af37, transparent: true, opacity: 0.06,
            blending: THREE.AdditiveBlending, depthWrite: false
        });
        rayLineSegments = new THREE.LineSegments(rayLineGeometry, lineMat);
        scene.add(rayLineSegments);
    }

    function getAttenuationColor(db) {
        if (db < 20) return new THREE.Color(0.27, 1.0, 0.27);
        if (db < 40) return new THREE.Color(1.0, 1.0, 0.27);
        if (db < 60) return new THREE.Color(1.0, 0.67, 0.27);
        return new THREE.Color(1.0, 0.27, 0.27);
    }

    function spawnParticlesFromPaths(paths) {
        particleData = [];
        var lineIdx = 0;

        paths.forEach(function (path, pathIndex) {
            if (path.path_points.length < 2) return;
            var color = getAttenuationColor(path.attenuation_db);

            for (var p = 0; p < path.path_points.length - 1; p++) {
                var start = path.path_points[p];
                var end = path.path_points[p + 1];

                if (particleData.length < MAX_PARTICLES) {
                    particleData.push({
                        x: start.x, y: start.y, z: start.z,
                        tx: end.x, ty: end.y, tz: end.z,
                        progress: Math.random() * 0.3,
                        speed: 0.008 + Math.random() * 0.008,
                        color: color,
                        life: 1.0,
                        segment: p,
                        pathIndex: pathIndex
                    });
                }

                if (lineIdx < MAX_PARTICLES * 6 - 5) {
                    rayLinePositions[lineIdx++] = start.x;
                    rayLinePositions[lineIdx++] = start.y;
                    rayLinePositions[lineIdx++] = start.z;
                    rayLinePositions[lineIdx++] = end.x;
                    rayLinePositions[lineIdx++] = end.y;
                    rayLinePositions[lineIdx++] = end.z;
                }
            }
        });

        activeParticleCount = particleData.length;
        particleInstances.count = activeParticleCount;

        var colorAttr = particleInstances.instanceColor;
        for (var i = 0; i < activeParticleCount; i++) {
            colorAttr.setXYZ(i, particleData[i].color.r, particleData[i].color.g, particleData[i].color.b);
        }
        colorAttr.needsUpdate = true;

        rayLineGeometry.attributes.position.needsUpdate = true;
        rayLineGeometry.setDrawRange(0, lineIdx / 3);

        soundPaths = paths;

        var rayEl = document.getElementById('ray-count');
        if (rayEl) rayEl.textContent = paths.length;
        var partEl = document.getElementById('particle-count');
        if (partEl) partEl.textContent = activeParticleCount;
    }

    function updateGPUParticles(dt) {
        if (!particleRunning || activeParticleCount === 0) return;
        var speedMult = simParams.speed / 5;
        var mat = particleInstances.instanceMatrix;
        var colAttr = particleInstances.instanceColor;
        var aliveCount = 0;

        for (var i = 0; i < activeParticleCount; i++) {
            var p = particleData[i];
            p.progress += p.speed * speedMult * dt * 0.06;

            if (p.progress >= 1) {
                var path = soundPaths[p.pathIndex];
                if (path && path.path_points.length > p.segment + 2) {
                    var next = path.path_points[p.segment + 1];
                    var nextEnd = path.path_points[p.segment + 2];
                    if (next && nextEnd) {
                        p.x = next.x; p.y = next.y; p.z = next.z;
                        p.tx = nextEnd.x; p.ty = nextEnd.y; p.tz = nextEnd.z;
                        p.progress = 0;
                        p.segment++;
                    }
                } else {
                    p.life -= 0.01 * dt;
                }
            }

            if (p.life <= 0) {
                particleDummy.position.set(0, -1000, 0);
                particleDummy.scale.set(0, 0, 0);
                particleDummy.updateMatrix();
                mat.setMatrixAt(i, particleDummy.matrix);
                colAttr.setXYZ(i, 0, 0, 0);
                continue;
            }

            var x = p.x + (p.tx - p.x) * p.progress;
            var y = p.y + (p.ty - p.y) * p.progress;
            var z = p.z + (p.tz - p.z) * p.progress;

            var s = 0.2 + p.life * 0.3;
            particleDummy.position.set(x, y, z);
            particleDummy.scale.set(s, s, s);
            particleDummy.updateMatrix();
            mat.setMatrixAt(i, particleDummy.matrix);

            var fade = Math.min(1, p.life);
            colAttr.setXYZ(i, p.color.r * fade, p.color.g * fade, p.color.b * fade);
            aliveCount++;
        }

        mat.needsUpdate = true;
        colAttr.needsUpdate = true;

        var partEl = document.getElementById('particle-count');
        if (partEl) partEl.textContent = aliveCount;
    }

    function animate() {
        requestAnimationFrame(animate);
        var now = performance.now();
        var dt = now - lastTime;
        lastTime = now;
        frameCount++;
        if (frameCount % 30 === 0) {
            var fpsEl = document.getElementById('fps');
            if (fpsEl) fpsEl.textContent = Math.round(1000 / (dt + 0.001));
        }
        controls.update();
        updateGPUParticles(dt);
        renderer.render(scene, camera);
    }

    function smoothCameraMove(tx, ty, tz, lx, ly, lz) {
        var start = { x: camera.position.x, y: camera.position.y, z: camera.position.z };
        var targetStart = { x: controls.target.x, y: controls.target.y, z: controls.target.z };
        var duration = 800;
        var t0 = performance.now();
        function step() {
            var p = Math.min(1, (performance.now() - t0) / duration);
            var ease = p < 0.5 ? 2 * p * p : -1 + (4 - 2 * p) * p;
            camera.position.x = start.x + (tx - start.x) * ease;
            camera.position.y = start.y + (ty - start.y) * ease;
            camera.position.z = start.z + (tz - start.z) * ease;
            controls.target.x = targetStart.x + (lx - targetStart.x) * ease;
            controls.target.y = targetStart.y + (ly - targetStart.y) * ease;
            controls.target.z = targetStart.z + (lz - targetStart.z) * ease;
            if (p < 1) requestAnimationFrame(step);
        }
        step();
    }

    function setView(mode) {
        var pos, look;
        if (mode === 'top') { pos = [0, 100, 0.01]; look = [0, 0, -10]; }
        else if (mode === 'front') { pos = [0, 20, 90]; look = [0, 2, -10]; }
        else if (mode === 'side') { pos = [90, 20, 0]; look = [0, 2, -10]; }
        else if (mode === 'perspective') { pos = [55, 45, 65]; look = [0, 2, -10]; }
        else { pos = [50, 45, 70]; look = [0, 2, -10]; }
        smoothCameraMove(pos[0], pos[1], pos[2], look[0], look[1], look[2]);
    }

    function selectSite(site) {
        currentSite = site;
        var btns = document.querySelectorAll('.site-btn');
        if (btns) {
            btns.forEach(function (b) {
                b.classList.toggle('active', b.dataset.site === site);
            });
        }
        var pos;
        if (site === 'huiyinbi') pos = { x: 0, y: 0, z: 0 };
        else if (site === 'sanyinshi') pos = { x: 0, y: 0, z: 5 };
        else pos = { x: 0, y: 0, z: -30 };
        smoothCameraMove(pos.x + 45, pos.y + 40, pos.z + 45, pos.x, pos.y + 2, pos.z);
    }

    function resizeRenderer() {
        var canvas = renderer.domElement;
        if (!canvas) return;
        var parent = canvas.parentElement;
        if (!parent) return;
        var w = parent.clientWidth;
        var h = parent.clientHeight;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h);
    }

    function initSoundField() {
        soundFieldCanvas = document.getElementById('sound-field-canvas');
        if (!soundFieldCanvas) return;
        soundFieldCtx = soundFieldCanvas.getContext('2d');
        soundFieldCanvas.width = soundFieldCanvas.clientWidth;
        soundFieldCanvas.height = soundFieldCanvas.clientHeight;
        drawEmptyField();
    }

    function drawEmptyField() {
        if (!soundFieldCtx) return;
        soundFieldCtx.fillStyle = 'rgba(0,0,0,0.8)';
        soundFieldCtx.fillRect(0, 0, soundFieldCanvas.width, soundFieldCanvas.height);
        soundFieldCtx.fillStyle = '#808070';
        soundFieldCtx.font = '13px "Microsoft YaHei"';
        soundFieldCtx.textAlign = 'center';
        soundFieldCtx.fillText('点击"计算波动声场"生成云图', soundFieldCanvas.width / 2, soundFieldCanvas.height / 2);
    }

    function drawSoundField(snapshot) {
        if (!soundFieldCtx) return;
        var w = soundFieldCanvas.width, h = soundFieldCanvas.height;
        var field = snapshot.pressure_field;
        if (!field || field.length === 0) return;
        var cols = field.length;
        var rows = field[0].length;
        soundFieldCtx.clearRect(0, 0, w, h);
        var cellW = w / cols, cellH = h / rows;
        for (var i = 0; i < cols; i++) {
            for (var j = 0; j < rows; j++) {
                var v = field[i][j];
                soundFieldCtx.fillStyle = pressureToColor(v);
                soundFieldCtx.fillRect(i * cellW, j * cellH, cellW + 1, cellH + 1);
            }
        }
        var sx = (cols / 2) * cellW;
        var sy = (rows / 2) * cellH;
        soundFieldCtx.beginPath();
        soundFieldCtx.arc(sx, sy, 6, 0, Math.PI * 2);
        soundFieldCtx.fillStyle = '#ffffff';
        soundFieldCtx.fill();
        soundFieldCtx.strokeStyle = '#d4af37';
        soundFieldCtx.lineWidth = 2;
        soundFieldCtx.stroke();
    }

    function pressureToColor(v) {
        v = Math.max(0, Math.min(1, v));
        var stops = [
            [0, [0, 0, 128]],
            [0.16, [0, 128, 255]],
            [0.33, [0, 255, 255]],
            [0.5, [128, 255, 128]],
            [0.66, [255, 255, 0]],
            [0.83, [255, 128, 0]],
            [1, [255, 0, 0]]
        ];
        for (var i = 0; i < stops.length - 1; i++) {
            if (v >= stops[i][0] && v <= stops[i + 1][0]) {
                var t = (v - stops[i][0]) / (stops[i + 1][0] - stops[i][0]);
                var c1 = stops[i][1], c2 = stops[i + 1][1];
                var r = Math.round(c1[0] + (c2[0] - c1[0]) * t);
                var g = Math.round(c1[1] + (c2[1] - c1[1]) * t);
                var b = Math.round(c1[2] + (c2[2] - c1[2]) * t);
                return 'rgba(' + r + ',' + g + ',' + b + ',0.9)';
            }
        }
        return '#000080';
    }

    function init(canvasId) {
        var canvas = document.getElementById(canvasId);
        if (!canvas) return;

        scene = new THREE.Scene();
        scene.background = new THREE.Color(0x0a0a1a);
        scene.fog = new THREE.FogExp2(0x0a0a1a, 0.008);

        camera = new THREE.PerspectiveCamera(60, canvas.clientWidth / canvas.clientHeight, 0.1, 1000);
        camera.position.set(50, 45, 70);

        renderer = new THREE.WebGLRenderer({ canvas: canvas, antialias: true, alpha: true });
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.setSize(canvas.clientWidth, canvas.clientHeight);
        renderer.shadowMap.enabled = true;

        controls = new THREE.OrbitControls(camera, canvas);
        controls.enableDamping = true;
        controls.dampingFactor = 0.08;
        controls.target.set(0, 2, -10);

        var ambient = new THREE.AmbientLight(0x404050, 0.6);
        scene.add(ambient);
        var sun = new THREE.DirectionalLight(0xffeedd, 1.2);
        sun.position.set(50, 80, 40);
        sun.castShadow = true;
        sun.shadow.mapSize.width = 2048;
        sun.shadow.mapSize.height = 2048;
        sun.shadow.camera.left = -80;
        sun.shadow.camera.right = 80;
        sun.shadow.camera.top = 80;
        sun.shadow.camera.bottom = -80;
        scene.add(sun);
        var fill = new THREE.DirectionalLight(0x8888ff, 0.3);
        fill.position.set(-30, 20, -40);
        scene.add(fill);

        buildGround();
        buildTiantanComplex();
        initGPUParticles();
        animate();
    }

    function getScene() { return scene; }
    function getCamera() { return camera; }
    function getControls() { return controls; }

    function getCurrentSite() { return currentSite; }
    function setCurrentSite(site) { currentSite = site; }

    function setSimParams(params) {
        if (params) {
            for (var k in params) {
                if (params.hasOwnProperty(k)) {
                    simParams[k] = params[k];
                }
            }
        }
    }
    function getSimParams() { return simParams; }

    function toggleParticles() {
        particleRunning = !particleRunning;
        var el = document.getElementById('particle-toggle');
        if (el) el.textContent = particleRunning ? '⏸ 暂停粒子动画' : '▶ 继续粒子动画';
    }
    function isParticleRunning() { return particleRunning; }

    function getSoundPaths() { return soundPaths; }

    global.TempleOfHeaven3D = {
        init: init,
        buildHuiyinbi: buildHuiyinbi,
        buildSanyinshi: buildSanyinshi,
        buildHuanqiutan: buildHuanqiutan,
        createSiteLabel: createSiteLabel,
        initGPUParticles: initGPUParticles,
        spawnParticlesFromPaths: spawnParticlesFromPaths,
        updateGPUParticles: updateGPUParticles,
        getAttenuationColor: getAttenuationColor,
        animate: animate,
        setView: setView,
        selectSite: selectSite,
        smoothCameraMove: smoothCameraMove,
        resizeRenderer: resizeRenderer,
        getScene: getScene,
        getCamera: getCamera,
        getControls: getControls,
        getCurrentSite: getCurrentSite,
        setCurrentSite: setCurrentSite,
        setSimParams: setSimParams,
        getSimParams: getSimParams,
        toggleParticles: toggleParticles,
        isParticleRunning: isParticleRunning,
        initSoundField: initSoundField,
        drawSoundField: drawSoundField,
        drawEmptyField: drawEmptyField,
        pressureToColor: pressureToColor,
        getSoundPaths: getSoundPaths,
        MAX_PARTICLES: MAX_PARTICLES,
        siteColors: siteColors
    };
})(window);

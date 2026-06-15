/**
 * 多朝代祭祀建筑与现代音乐厅3D建模模块
 * 依赖: THREE.js
 */

const Buildings3D = (function() {
    const buildings = {};
    let scene = null;
    let camera = null;
    let activeBuildingId = null;
    let dynastyGroup = null;
    let concertHallGroup = null;
    let templeVisibility = true;

    function init(sceneRef, cameraRef) {
        scene = sceneRef;
        camera = cameraRef || null;
        dynastyGroup = new THREE.Group();
        dynastyGroup.name = 'dynasty-buildings';
        dynastyGroup.visible = false;
        scene.add(dynastyGroup);

        concertHallGroup = new THREE.Group();
        concertHallGroup.name = 'concert-hall-buildings';
        concertHallGroup.visible = false;
        scene.add(concertHallGroup);
    }

    function createDynastyHall(type, position) {
        const group = new THREE.Group();
        group.position.set(position.x, position.y, position.z);

        const configs = {
            tang: {
                width: 88, height: 29, depth: 88,
                color: 0x8B4513, roofColor: 0x1a1a2e,
                style: 'tang',
                pillarCount: 12,
                baseHeight: 2.5
            },
            song: {
                width: 48, height: 25, depth: 20,
                color: 0xA0522D, roofColor: 0x2d3436,
                style: 'song',
                pillarCount: 10,
                baseHeight: 2.0
            },
            ming: {
                width: 60, height: 27, depth: 35,
                color: 0xCD853F, roofColor: 0x2c3e50,
                style: 'ming',
                pillarCount: 14,
                baseHeight: 3.0
            },
            qing: {
                width: 64, height: 26.92, depth: 37,
                color: 0xD2691E, roofColor: 0x1a1a40,
                style: 'qing',
                pillarCount: 16,
                baseHeight: 3.5
            }
        };

        const config = configs[type] || configs.qing;

        const baseGeo = new THREE.BoxGeometry(config.width + 4, config.baseHeight, config.depth + 4);
        const baseMat = new THREE.MeshStandardMaterial({
            color: 0xd4c4a8,
            roughness: 0.8,
            metalness: 0.1
        });
        const base = new THREE.Mesh(baseGeo, baseMat);
        base.position.y = config.baseHeight / 2;
        group.add(base);

        const stepGeo = new THREE.BoxGeometry(8, 0.3, 3);
        const stepMat = new THREE.MeshStandardMaterial({ color: 0xc0b280 });
        for (let i = 0; i < 3; i++) {
            const step = new THREE.Mesh(stepGeo, stepMat);
            step.position.set(0, i * 0.3 + config.baseHeight + 0.15, config.depth / 2 + 1.5 - i * 0.5);
            group.add(step);
        }

        const hallWidth = config.width - 4;
        const hallDepth = config.depth - 4;
        const hallHeight = config.height - config.baseHeight;

        const wallMat = new THREE.MeshStandardMaterial({
            color: config.color,
            roughness: 0.7,
            metalness: 0.1
        });
        const wallThickness = 0.8;

        const frontWall = new THREE.Mesh(
            new THREE.BoxGeometry(hallWidth, hallHeight * 0.6, wallThickness),
            wallMat
        );
        frontWall.position.set(0, config.baseHeight + hallHeight * 0.3, hallDepth / 2 - wallThickness / 2);
        group.add(frontWall);

        const backWall = new THREE.Mesh(
            new THREE.BoxGeometry(hallWidth, hallHeight * 0.7, wallThickness),
            wallMat
        );
        backWall.position.set(0, config.baseHeight + hallHeight * 0.35, -hallDepth / 2 + wallThickness / 2);
        group.add(backWall);

        const leftWall = new THREE.Mesh(
            new THREE.BoxGeometry(wallThickness, hallHeight * 0.65, hallDepth),
            wallMat
        );
        leftWall.position.set(-hallWidth / 2 + wallThickness / 2, config.baseHeight + hallHeight * 0.325, 0);
        group.add(leftWall);

        const rightWall = new THREE.Mesh(
            new THREE.BoxGeometry(wallThickness, hallHeight * 0.65, hallDepth),
            wallMat
        );
        rightWall.position.set(hallWidth / 2 - wallThickness / 2, config.baseHeight + hallHeight * 0.325, 0);
        group.add(rightWall);

        const pillarMat = new THREE.MeshStandardMaterial({
            color: 0x5C3317,
            roughness: 0.6,
            metalness: 0.2
        });
        const pillarRadius = type === 'tang' ? 0.6 : 0.5;
        const pillarHeight = hallHeight * 0.7;
        const pillarGeo = new THREE.CylinderGeometry(pillarRadius, pillarRadius * 1.1, pillarHeight, 16);

        const pillarPositionsX = [];
        for (let i = 0; i < config.pillarCount; i++) {
            const x = -hallWidth / 2 + 2 + (i * (hallWidth - 4) / (config.pillarCount - 1));
            pillarPositionsX.push(x);
        }

        pillarPositionsX.forEach((x, idx) => {
            const frontPillar = new THREE.Mesh(pillarGeo, pillarMat);
            frontPillar.position.set(x, config.baseHeight + pillarHeight / 2, hallDepth / 2 - 2);
            group.add(frontPillar);

            const backPillar = new THREE.Mesh(pillarGeo, pillarMat);
            backPillar.position.set(x, config.baseHeight + pillarHeight / 2, -hallDepth / 2 + 2);
            group.add(backPillar);

            if (idx % 2 === 0) {
                const midPillar = new THREE.Mesh(pillarGeo, pillarMat);
                midPillar.position.set(x, config.baseHeight + pillarHeight / 2, 0);
                group.add(midPillar);
            }
        });

        const roofHeight = hallHeight * 0.4;
        const roofGeo = new THREE.ConeGeometry(
            Math.max(hallWidth, hallDepth) / 2 + 3,
            roofHeight,
            4
        );
        const roofMat = new THREE.MeshStandardMaterial({
            color: config.roofColor,
            roughness: 0.4,
            metalness: 0.3,
            side: THREE.DoubleSide
        });
        const roof = new THREE.Mesh(roofGeo, roofMat);
        roof.position.y = config.baseHeight + hallHeight * 0.75 + roofHeight / 2;
        roof.rotation.y = Math.PI / 4;
        group.add(roof);

        const eaveOverhang = 2 + (type === 'tang' ? 2 : type === 'song' ? 1.5 : 1);
        const eaveGeo = new THREE.BoxGeometry(hallWidth + eaveOverhang * 2, 0.3, hallDepth + eaveOverhang * 2);
        const eave = new THREE.Mesh(eaveGeo, roofMat);
        eave.position.y = config.baseHeight + hallHeight * 0.7;
        group.add(eave);

        if (type === 'qing' || type === 'ming') {
            const dragonGeo = new THREE.SphereGeometry(1.2, 16, 16);
            const dragonMat = new THREE.MeshStandardMaterial({
                color: 0xffd700,
                emissive: 0xffa500,
                emissiveIntensity: 0.3,
                metalness: 0.9,
                roughness: 0.2
            });
            const dragon = new THREE.Mesh(dragonGeo, dragonMat);
            dragon.position.y = config.baseHeight + hallHeight * 0.75 + roofHeight + 0.5;
            dragon.scale.set(1, 0.6, 1);
            group.add(dragon);
        }

        const caissonGeo = new THREE.CircleGeometry(4, 32);
        const caissonMat = new THREE.MeshStandardMaterial({
            color: 0x1a1a40,
            emissive: 0x2a2a50,
            emissiveIntensity: 0.2,
            side: THREE.DoubleSide
        });
        const caisson = new THREE.Mesh(caissonGeo, caissonMat);
        caisson.rotation.x = -Math.PI / 2;
        caisson.position.y = config.baseHeight + hallHeight * 0.65;
        group.add(caisson);

        const rings = 3;
        for (let i = 0; i < rings; i++) {
            const ringGeo = new THREE.TorusGeometry(2 + i * 0.8, 0.08, 8, 48);
            const ringMat = new THREE.MeshStandardMaterial({
                color: 0xffd700,
                emissive: 0xffa500,
                emissiveIntensity: 0.2
            });
            const ring = new THREE.Mesh(ringGeo, ringMat);
            ring.rotation.x = -Math.PI / 2;
            ring.position.y = config.baseHeight + hallHeight * 0.65 + 0.1 + i * 0.15;
            group.add(ring);
        }

        const floorMat = new THREE.MeshStandardMaterial({
            color: type === 'ming' || type === 'qing' ? 0xd4af37 : 0x808080,
            roughness: 0.3,
            metalness: 0.4
        });
        const floorGeo = new THREE.PlaneGeometry(hallWidth - 2, hallDepth - 2);
        const floor = new THREE.Mesh(floorGeo, floorMat);
        floor.rotation.x = -Math.PI / 2;
        floor.position.y = config.baseHeight + 0.05;
        group.add(floor);

        group.userData = {
            buildingType: type,
            config: config
        };

        return group;
    }

    function createConcertHall(type, position) {
        const group = new THREE.Group();
        group.position.set(position.x, position.y, position.z);

        const configs = {
            shoemaker: {
                width: 20, height: 18, depth: 45,
                wallColor: 0x8B4513,
                ceilingColor: 0x654321,
                style: 'shoebox',
                balconyCount: 2
            },
            vineyard: {
                width: 25, height: 15, depth: 40,
                wallColor: 0x5D4037,
                ceilingColor: 0x3E2723,
                style: 'vineyard',
                terraceCount: 4
            },
            boston: {
                width: 23.2, height: 18.6, depth: 39.6,
                wallColor: 0xD2B48C,
                ceilingColor: 0xF5DEB3,
                style: 'classical',
                balconyCount: 2
            }
        };

        const config = configs[type] || configs.shoemaker;

        const wallMat = new THREE.MeshStandardMaterial({
            color: config.wallColor,
            roughness: 0.7,
            metalness: 0.1
        });
        const wallThickness = 0.5;

        const leftWall = new THREE.Mesh(
            new THREE.BoxGeometry(wallThickness, config.height, config.depth),
            wallMat
        );
        leftWall.position.set(-config.width / 2 + wallThickness / 2, config.height / 2, 0);
        group.add(leftWall);

        const rightWall = new THREE.Mesh(
            new THREE.BoxGeometry(wallThickness, config.height, config.depth),
            wallMat
        );
        rightWall.position.set(config.width / 2 - wallThickness / 2, config.height / 2, 0);
        group.add(rightWall);

        const backWall = new THREE.Mesh(
            new THREE.BoxGeometry(config.width, config.height, wallThickness),
            wallMat
        );
        backWall.position.set(0, config.height / 2, -config.depth / 2 + wallThickness / 2);
        group.add(backWall);

        const ceilingMat = new THREE.MeshStandardMaterial({
            color: config.ceilingColor,
            roughness: 0.6,
            metalness: 0.1
        });
        const ceiling = new THREE.Mesh(
            new THREE.BoxGeometry(config.width - 1, 0.5, config.depth - 1),
            ceilingMat
        );
        ceiling.position.y = config.height - 0.25;
        group.add(ceiling);

        if (config.style === 'shoebox' || config.style === 'classical') {
            const diffuserMat = new THREE.MeshStandardMaterial({
                color: 0x654321,
                roughness: 0.8
            });

            for (let side = 0; side < 2; side++) {
                const xSign = side === 0 ? -1 : 1;
                for (let i = 0; i < 8; i++) {
                    const z = -config.depth / 2 + 3 + i * 5;
                    const diffuser = new THREE.Mesh(
                        new THREE.BoxGeometry(0.3, 2, 1.5),
                        diffuserMat
                    );
                    diffuser.position.set(
                        xSign * (config.width / 2 - 1),
                        config.height * 0.5,
                        z
                    );
                    group.add(diffuser);
                }
            }

            for (let i = 0; i < config.balconyCount; i++) {
                const balconyDepth = 3;
                const balconyHeight = 1.2;
                const yPos = config.height * (0.4 + i * 0.25);
                const balconyGeo = new THREE.BoxGeometry(config.width - 4, balconyHeight, balconyDepth);
                const balconyMat = new THREE.MeshStandardMaterial({ color: 0x5D4037 });
                const balcony = new THREE.Mesh(balconyGeo, balconyMat);
                balcony.position.set(0, yPos, config.depth / 2 - 3 - i * 4);
                group.add(balcony);
            }
        }

        if (config.style === 'vineyard') {
            const terraceMat = new THREE.MeshStandardMaterial({
                color: 0x5D4037,
                roughness: 0.7
            });
            const seatMat = new THREE.MeshStandardMaterial({
                color: 0x8B0000,
                roughness: 0.9
            });

            const stageWidth = 12;
            const stageDepth = 8;
            const stageGeo = new THREE.BoxGeometry(stageWidth, 1, stageDepth);
            const stageMat = new THREE.MeshStandardMaterial({ color: 0x8B4513 });
            const stage = new THREE.Mesh(stageGeo, stageMat);
            stage.position.set(0, 0.5, -config.depth / 2 + 8);
            group.add(stage);

            for (let tier = 0; tier < config.terraceCount; tier++) {
                const tierWidth = config.width - tier * 3;
                const tierDepth = 4;
                const yOffset = tier * 1.5;
                const zOffset = -config.depth / 2 + 14 + tier * 6;

                const tierGeo = new THREE.BoxGeometry(tierWidth, 1, tierDepth);
                const tierMesh = new THREE.Mesh(tierGeo, terraceMat);
                tierMesh.position.set(0, yOffset + 0.5, zOffset);
                group.add(tierMesh);

                const seatsPerRow = Math.floor(tierWidth / 1);
                for (let s = 0; s < seatsPerRow; s++) {
                    const seatX = -tierWidth / 2 + 0.5 + s;
                    const seatGeo = new THREE.BoxGeometry(0.5, 0.6, 0.5);
                    const seat = new THREE.Mesh(seatGeo, seatMat);
                    seat.position.set(seatX, yOffset + 1.3, zOffset);
                    group.add(seat);
                }
            }
        } else {
            const stageWidth = config.width * 0.6;
            const stageDepth = 6;
            const stageGeo = new THREE.BoxGeometry(stageWidth, 1, stageDepth);
            const stageMat = new THREE.MeshStandardMaterial({ color: 0x8B4513 });
            const stage = new THREE.Mesh(stageGeo, stageMat);
            stage.position.set(0, 0.5, -config.depth / 2 + 5);
            group.add(stage);

            const seatMat = new THREE.MeshStandardMaterial({
                color: 0x8B0000,
                roughness: 0.9
            });

            const rows = 15;
            const seatsPerRow = Math.floor(config.width * 0.8 / 1);
            for (let r = 0; r < rows; r++) {
                const zOffset = -config.depth / 2 + 12 + r * 2;
                const rowWidth = config.width * 0.8 + r * 0.5;
                const seatsInRow = Math.floor(rowWidth / 1);
                for (let s = 0; s < seatsInRow; s++) {
                    const seatX = -rowWidth / 2 + 0.5 + s;
                    const seatGeo = new THREE.BoxGeometry(0.5, 0.8, 0.5);
                    const seat = new THREE.Mesh(seatGeo, seatMat);
                    seat.position.set(seatX, 0.4, zOffset);
                    group.add(seat);
                }
            }
        }

        const canopyGeo = new THREE.CircleGeometry(config.width * 0.35, 32);
        const canopyMat = new THREE.MeshStandardMaterial({
            color: 0xD2B48C,
            side: THREE.DoubleSide,
            roughness: 0.5,
            metalness: 0.2
        });
        const canopy = new THREE.Mesh(canopyGeo, canopyMat);
        canopy.rotation.x = -Math.PI / 2;
        canopy.position.set(0, config.height * 0.7, -config.depth / 2 + 5);
        group.add(canopy);

        const supportCount = 4;
        const supportMat = new THREE.MeshStandardMaterial({ color: 0x696969, metalness: 0.8 });
        for (let i = 0; i < supportCount; i++) {
            const angle = (i / supportCount) * Math.PI * 2;
            const r = config.width * 0.3;
            const support = new THREE.Mesh(
                new THREE.CylinderGeometry(0.08, 0.08, config.height * 0.3, 8),
                supportMat
            );
            support.position.set(
                Math.cos(angle) * r,
                config.height * 0.7 + config.height * 0.15,
                -config.depth / 2 + 5 + Math.sin(angle) * r * 0.6
            );
            group.add(support);
        }

        const floorMat = new THREE.MeshStandardMaterial({
            color: 0x4a3728,
            roughness: 0.8
        });
        const floor = new THREE.Mesh(
            new THREE.PlaneGeometry(config.width - 2, config.depth - 2),
            floorMat
        );
        floor.rotation.x = -Math.PI / 2;
        floor.position.y = 0.02;
        group.add(floor);

        group.userData = {
            buildingType: type,
            config: config
        };

        return group;
    }

    function createBuilding(buildingId, position) {
        const buildingType = buildingId.replace('_temple', '').replace('_hall', '');
        const isAncient = buildingId.includes('temple');

        let building;
        if (isAncient) {
            building = createDynastyHall(buildingType, position);
        } else {
            building = createConcertHall(buildingType, position);
        }

        building.userData.buildingId = buildingId;
        buildings[buildingId] = building;
        return building;
    }

    function showBuilding(buildingId) {
        Object.values(buildings).forEach(b => {
            b.visible = false;
        });

        if (buildings[buildingId]) {
            buildings[buildingId].visible = true;
            activeBuildingId = buildingId;
        }
    }

    function getBuilding(buildingId) {
        return buildings[buildingId];
    }

    function getAllBuildings() {
        return buildings;
    }

    function getActiveBuildingId() {
        return activeBuildingId;
    }

    function createNoiseMarker(position, level) {
        const height = Math.max(0.5, (level - 40) / 40 * 3);
        const color = level > 75 ? 0xff4444 : level > 60 ? 0xffaa00 : 0x44aa44;

        const geo = new THREE.CylinderGeometry(0.5, 0.8, height, 12);
        const mat = new THREE.MeshStandardMaterial({
            color: color,
            emissive: color,
            emissiveIntensity: 0.3,
            transparent: true,
            opacity: 0.8
        });
        const marker = new THREE.Mesh(geo, mat);
        marker.position.set(position.x, height / 2 + 1.5, position.z);

        const ringGeo = new THREE.RingGeometry(1, 1.5, 32);
        const ringMat = new THREE.MeshBasicMaterial({
            color: color,
            side: THREE.DoubleSide,
            transparent: true,
            opacity: 0.4
        });
        const ring = new THREE.Mesh(ringGeo, ringMat);
        ring.rotation.x = -Math.PI / 2;
        ring.position.y = 1.52;
        marker.add(ring);

        marker.userData.noiseLevel = level;
        return marker;
    }

    function createSpeakerMarker(position, isActive) {
        const geo = new THREE.ConeGeometry(0.4, 1.2, 8);
        const color = isActive ? 0x00ff88 : 0x888888;
        const mat = new THREE.MeshStandardMaterial({
            color: color,
            emissive: isActive ? 0x00ff44 : 0x222222,
            emissiveIntensity: isActive ? 0.5 : 0
        });
        const marker = new THREE.Mesh(geo, mat);
        marker.position.set(position.x, 2.5, position.z);
        marker.rotation.z = Math.PI;

        const ballGeo = new THREE.SphereGeometry(0.3, 16, 16);
        const ball = new THREE.Mesh(ballGeo, mat);
        ball.position.y = 0.6;
        marker.add(ball);

        return marker;
    }

    function createListenerMarker(position) {
        const geo = new THREE.CylinderGeometry(0.25, 0.3, 1.7, 8);
        const mat = new THREE.MeshStandardMaterial({
            color: 0x4488ff,
            emissive: 0x2244aa,
            emissiveIntensity: 0.3
        });
        const marker = new THREE.Mesh(geo, mat);
        marker.position.set(position.x, 0.85, position.z);

        const headGeo = new THREE.SphereGeometry(0.25, 16, 16);
        const head = new THREE.Mesh(headGeo, mat);
        head.position.y = 0.95;
        marker.add(head);

        return marker;
    }

    function createLabelSprite(text) {
        const canvas = document.createElement('canvas');
        const ctx = canvas.getContext('2d');
        canvas.width = 256;
        canvas.height = 64;

        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(0, 0, 256, 64);
        ctx.strokeStyle = '#ffd700';
        ctx.lineWidth = 2;
        ctx.strokeRect(1, 1, 254, 62);

        ctx.fillStyle = '#ffd700';
        ctx.font = 'bold 20px "Microsoft YaHei", sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(text, 128, 32);

        const texture = new THREE.CanvasTexture(canvas);
        const material = new THREE.SpriteMaterial({
            map: texture,
            transparent: true
        });
        const sprite = new THREE.Sprite(material);
        sprite.scale.set(8, 2, 1);
        return sprite;
    }

    function showDynastyBuildings() {
        if (!dynastyGroup || !scene) return;

        if (dynastyGroup.children.length === 0) {
            const dynasties = ['tang', 'song', 'ming', 'qing'];
            const positions = [
                { x: -80, y: 0, z: -40 },
                { x: -80, y: 0, z: 40 },
                { x: 80, y: 0, z: -40 },
                { x: 80, y: 0, z: 40 }
            ];
            const labels = {
                tang: '唐代明堂',
                song: '宋代大庆殿',
                ming: '明代奉天殿',
                qing: '清代太和殿'
            };

            dynasties.forEach((dynasty, i) => {
                const building = createDynastyHall(dynasty, positions[i]);
                building.userData.buildingId = `${dynasty}_temple`;
                dynastyGroup.add(building);

                const labelSprite = createLabelSprite(labels[dynasty]);
                labelSprite.position.set(positions[i].x, 32, positions[i].z);
                dynastyGroup.add(labelSprite);

                buildings[`${dynasty}_temple`] = building;
            });
        }

        dynastyGroup.visible = true;
        if (concertHallGroup) concertHallGroup.visible = false;
        templeVisibility = false;

        if (camera) {
            camera.position.set(0, 80, 150);
            if (typeof TempleOfHeaven3D !== 'undefined' && TempleOfHeaven3D.controls) {
                TempleOfHeaven3D.controls.target.set(0, 10, 0);
                TempleOfHeaven3D.controls.update();
            }
        }
    }

    function showConcertHalls() {
        if (!concertHallGroup || !scene) return;

        if (concertHallGroup.children.length === 0) {
            const halls = ['shoemaker', 'vineyard', 'boston'];
            const positions = [
                { x: -60, y: 0, z: -30 },
                { x: 0, y: 0, z: 30 },
                { x: 60, y: 0, z: -30 }
            ];
            const labels = {
                shoemaker: '鞋盒式音乐厅',
                vineyard: '葡萄园式音乐厅',
                boston: '波士顿交响乐厅'
            };

            halls.forEach((hall, i) => {
                const building = createConcertHall(hall, positions[i]);
                building.userData.buildingId = `${hall}_hall`;
                concertHallGroup.add(building);

                const labelSprite = createLabelSprite(labels[hall]);
                labelSprite.position.set(positions[i].x, 22, positions[i].z);
                concertHallGroup.add(labelSprite);

                buildings[`${hall}_hall`] = building;
            });
        }

        concertHallGroup.visible = true;
        if (dynastyGroup) dynastyGroup.visible = false;
        templeVisibility = false;

        if (camera) {
            camera.position.set(0, 60, 120);
            if (typeof TempleOfHeaven3D !== 'undefined' && TempleOfHeaven3D.controls) {
                TempleOfHeaven3D.controls.target.set(0, 5, 0);
                TempleOfHeaven3D.controls.update();
            }
        }
    }

    function showTempleOnly() {
        if (dynastyGroup) dynastyGroup.visible = false;
        if (concertHallGroup) concertHallGroup.visible = false;
        templeVisibility = true;
    }

    function getScene() {
        return scene;
    }

    return {
        init,
        createBuilding,
        showBuilding,
        getBuilding,
        getAllBuildings,
        getActiveBuildingId,
        createNoiseMarker,
        createSpeakerMarker,
        createListenerMarker,
        createDynastyHall,
        createConcertHall,
        showDynastyBuildings,
        showConcertHalls,
        showTempleOnly,
        getScene,
        createLabelSprite
    };
})();

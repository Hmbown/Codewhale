/* Generated from pet/src/core. Run npm --prefix pet run sync. */
(function(global){
'use strict';
if(!global.structuredClone)global.structuredClone=value=>JSON.parse(JSON.stringify(value));
const factories={},cache={};
factories["pet-native"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetNative = void 0;
const pet_world_js_1 = require("./pet-world.js");
const pet_telemetry_js_1 = require("./pet-telemetry.js");
const pet_sim_js_1 = require("./pet-sim.js");
const pet_audio_js_1 = require("./pet-audio.js");
const pet_engine_js_1 = require("./pet-engine.js");
/** Synchronous JSON boundary for JavaScriptCore and embedded JS runtimes.
 * Native hosts share the actual world / score implementation, not a rewrite. */
class PetNative {
    world;
    engine = new pet_engine_js_1.PetEngineTelemetry();
    engineTick = 0;
    segment;
    liveTape = new pet_telemetry_js_1.PetLiveTape();
    constructor(pointsJSON, tapeJSONL = '', interactionsJSON = '[]', live = false, expressionVersion = 2) {
        const points = JSON.parse(pointsJSON);
        if (!Array.isArray(points) || points.length !== 980 || points.some(p => !Array.isArray(p) || p.length !== 2 || !p.every(n => Number.isFinite(n) && Math.abs(n) <= 1)))
            throw new Error('Invalid native whale body.');
        this.world = new pet_world_js_1.PetWorld(points, live ? (0, pet_telemetry_js_1.compilePetTelemetry)([]) : (0, pet_telemetry_js_1.decodePetJSONL)(tapeJSONL), JSON.parse(interactionsJSON), expressionVersion, true);
    }
    step(dt, motion) { this.world.step(dt, { motion, sensitivity: 1 }); return this.snapshot(); }
    snapshot() { return JSON.stringify({ ...this.world.frame, voices: this.world.voices, digest: (0, pet_sim_js_1.digest)(this.world.sim) }); }
    interact(kind, x, y) { this.world.interact(kind, x, y); }
    interactions() { return JSON.stringify(this.world.interactions); }
    accept(packet) { this.world.acceptTelemetry(JSON.parse(packet)); }
    acceptLiveTail(text) {
        const packet = this.liveTape.readTail(text);
        if (!packet)
            return false;
        this.world.acceptTelemetry(packet);
        return true;
    }
    resetLiveInput() { this.liveTape.reset(); return this.resumeEngine(); }
    recording(withCheckpoint = false) { return JSON.stringify(this.world.recording(withCheckpoint)); }
    needsSegment() { return this.world.needsSegment; }
    prepareSegment() { this.segment = this.world.prepareSegment(); return JSON.stringify(this.segment.recording); }
    commitSegment() { if (!this.segment)
        throw new Error('No pet segment was prepared.'); this.segment.commit(); this.segment = undefined; }
    checkpoint() { return JSON.stringify(this.world.checkpoint()); }
    recordingChunk(index, completed = false) { return this.world.recordingChunk(index, completed); }
    restoreCheckpoint(text) {
        if (text.length > 512 * 1024)
            throw new Error('Pet checkpoint exceeds its size limit.');
        this.restoreRecording(JSON.stringify({ ...this.world.recording(false), checkpoint: JSON.parse(text) }));
    }
    restoreRecording(text) {
        if (text.length > 8 * 1024 * 1024)
            throw new Error('Native habitat exceeds 8 MiB.');
        const points = this.world.sim.p.map(p => [p.hx, p.hy]);
        this.world = pet_world_js_1.PetWorld.fromRecording(points, JSON.parse(text));
        this.liveTape.reset();
        this.segment = undefined;
        this.engineTick = Math.round(this.world.frame.timeMs * 30 / 1000);
        this.engine = new pet_engine_js_1.PetEngineTelemetry();
    }
    /** Resume a live host at the first unrecorded bucket. Keep every accepted
     * interval, but never present its last observed frame as current evidence. */
    resumeEngine() {
        this.world.resumeObservation();
        this.engineTick = Math.round(this.world.frame.timeMs * 30 / 1000);
        this.engine = new pet_engine_js_1.PetEngineTelemetry();
        this.world.voices = [];
        return this.world.frame.timeMs;
    }
    observeEngine(metadataJSON, timeMs) { this.engine.observe(JSON.parse(metadataJSON), timeMs); }
    advanceEngine(timeMs, motion, waiting) {
        const target = Math.floor(timeMs * 30 / 1000 + 1e-8);
        if (!Number.isFinite(timeMs) || target < this.engineTick || target - this.engineTick > 300)
            throw new Error('Engine pet clock jump.');
        this.engine.confirmWaiting(timeMs, waiting);
        const voices = [];
        while (this.engineTick < target) {
            if ((this.engineTick + 1) % 12 === 0)
                this.world.acceptTelemetry(this.engine.bucket(Math.floor(this.engineTick / 12)));
            this.world.step(1 / 30, { motion, sensitivity: 1 });
            voices.push(...this.world.voices);
            this.engineTick++;
        }
        this.world.voices = voices;
    }
    terminal(width, height) {
        if (![width, height].every(n => Number.isSafeInteger(n) && n >= 1 && n <= 512))
            throw new Error('Invalid pet raster size.');
        const { sim, frame } = this.world, l = (0, pet_sim_js_1.layout)(width * 2, height * 4, frame.state);
        const cells = Array(width * height).fill(0), bits = [[1, 8], [2, 16], [4, 32], [64, 128]];
        sim.p.forEach((p, i) => {
            if (frame.state.observed < .92 && i % 2 === 1)
                return;
            const x = Math.round(l.ox + p.x * l.scale * l.flipX), y = Math.round(l.oy + p.y * l.scale);
            if (x >= 0 && x < width * 2 && y >= 0 && y < height * 4)
                cells[Math.floor(y / 4) * width + Math.floor(x / 2)] |= bits[y % 4][x % 2];
        });
        return JSON.stringify({ width, height, cells, timeMs: frame.timeMs, channel: frame.state.channel, arch: pet_sim_js_1.ARCH_OF[frame.state.channel],
            hollow: frame.state.observed < .92, dozing: frame.behaviour === 'doze', lit: frame.state.lit });
    }
    pcm(voicesJSON, startSample, length, rate) {
        const p = (0, pet_audio_js_1.renderPetPCM)(JSON.parse(voicesJSON), startSample, length, rate);
        return JSON.stringify([Array.from(p.left), Array.from(p.right)]);
    }
}
exports.PetNative = PetNative;

};
factories["pet-world"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetWorld = exports.PET_MAX_SECONDS = void 0;
const model_js_1 = require("./model.js");
const pet_sim_js_1 = require("./pet-sim.js");
const pet_telemetry_js_1 = require("./pet-telemetry.js");
const pet_audio_js_1 = require("./pet-audio.js");
const HZ = 30;
var pet_sim_js_2 = require("./pet-sim.js");
Object.defineProperty(exports, "PET_MAX_SECONDS", { enumerable: true, get: function () { return pet_sim_js_2.PET_MAX_SECONDS; } });
const seedFor = (name) => (0xC0FFEE ^ (0, model_js_1.stableHash)(name)) >>> 0;
/** Fixed-tick creature controller. Wall clocks and pointer APIs belong to drivers.
 * Reconstructing with the same tape and interactions is also the seek operation.
 * Particle, behaviour and identity randomness never consume one another's stream. */
class PetWorld {
    sim;
    tapeLog;
    tapeHashes = [];
    get tape() { return this.tapeLog; }
    interactionLog;
    get interactions() { return this.interactionLog.map(e => ({ ...e })); }
    frame;
    voices = [];
    score = new pet_audio_js_1.PetScore();
    accumulator = 0;
    tick = 0;
    bucketIndex = -1;
    interactionIndex = 0;
    branchTick = -1;
    random = (0, pet_sim_js_1.mulberry32)(seedFor('behaviour'));
    behaviour = 'swim';
    until = 6;
    targetX = .35;
    targetY = -.08;
    x = 0;
    y = 0;
    flip = 1;
    lit = 1;
    lastActivity = 0;
    addressedAt = -Infinity;
    waitSince = -1;
    food = null;
    members = new Map();
    lastStill = '';
    origin;
    segmented = false;
    hasTelemetry = false;
    get startTimeMs() { return this.origin?.frame.timeMs ?? 0; }
    get endTimeMs() { return Math.max(this.frame.timeMs, (this.tapeLog.at(-1)?.simTimeMs ?? 0) + 400); }
    get needsSegment() {
        return !this.segmented && this.tapeLog.length < 1024 && this.interactionLog.length < 4096
            || this.bucketIndex >= 1024 || this.interactionIndex >= 4096;
    }
    constructor(points, tape = [], interactions = [], expressionVersion = 2, segmented = false) {
        if (tape.length > 216_000 || interactions.length > 100_000)
            throw new Error('Pet recording exceeds its input limit.');
        this.sim = new pet_sim_js_1.PetSim(points, 0xC0FFEE, expressionVersion);
        this.segmented = segmented;
        this.hasTelemetry = tape.length > 0;
        this.tapeLog = structuredClone([...tape]);
        this.interactionLog = structuredClone([...interactions]);
        for (let i = 0; i < this.tape.length; i++) {
            const b = this.tape[i];
            (0, pet_telemetry_js_1.validatePetBucket)(b);
            if (segmented ? i > 0 && b.sequence <= this.tape[i - 1].sequence : b.sequence !== i)
                throw new Error('World requires contiguous version 1 pet buckets.');
        }
        for (let i = 0; i < this.interactionLog.length; i++) {
            const e = this.interactionLog[i];
            if (!Number.isFinite(e.timeMs) || e.timeMs < 0 || i > 0 && e.timeMs < this.interactionLog[i - 1].timeMs
                || !['attention', 'food'].includes(e.kind) || !Number.isFinite(e.x) || !Number.isFinite(e.y)
                || Math.abs(e.x) > 1 || Math.abs(e.y) > 1)
                throw new Error('Invalid pet interaction.');
        }
        this.hashTape(0);
        this.frame = this.makeFrame(0);
        this.voices = this.score.voices(this.frame);
        if (segmented)
            this.origin = this.checkpoint();
    }
    checkpoint() {
        return structuredClone({ petCheckpointVersion: this.segmented ? 2 : 1,
            ...(this.segmented ? { historyStart: (this.origin?.tick ?? this.tick), hasTelemetry: this.hasTelemetry } : {}), history: this.historyDigest(),
            sim: this.sim.checkpoint(), score: this.score.checkpoint(), accumulator: this.accumulator,
            tick: this.tick, bucketIndex: this.bucketIndex, interactionIndex: this.interactionIndex, branchTick: this.branchTick,
            random: this.random.state(), behaviour: this.behaviour, until: this.until,
            targetX: this.targetX, targetY: this.targetY, x: this.x, y: this.y, flip: this.flip, lit: this.lit,
            lastActivity: this.lastActivity, addressedAt: Number.isFinite(this.addressedAt) ? this.addressedAt : null,
            waitSince: this.waitSince, food: this.food, members: [...this.members.values()], lastStill: this.lastStill,
            frame: this.frame, voices: this.voices });
    }
    recording(withCheckpoint = true, completed = false) {
        const tapeEnd = completed ? this.bucketIndex + 1 : this.tapeLog.length;
        const inputEnd = completed ? this.interactionIndex : this.interactionLog.length;
        const history = this.historyDigest(tapeEnd, inputEnd);
        return { petReplayVersion: this.segmented ? 2 : 1, expressionVersion: this.sim.expressionVersion,
            tape: this.tapeLog.slice(0, tapeEnd), interactions: this.interactionLog.slice(0, inputEnd).map(e => ({ ...e })),
            ...(this.origin ? { start: { ...structuredClone(this.origin), hasTelemetry: this.hasTelemetry, history } } : {}),
            ...(withCheckpoint ? { checkpoint: { ...this.checkpoint(), history } } : {}) };
    }
    static fromRecording(points, value) {
        const r = value;
        if (!r || ![1, 2].includes(r.petReplayVersion) || !Array.isArray(r.tape) || !Array.isArray(r.interactions))
            throw new Error('Invalid pet recording.');
        const version = r.expressionVersion === undefined ? 1 : r.expressionVersion;
        if (![1, 2].includes(version))
            throw new Error('Unsupported pet expression version.');
        if (r.checkpoint !== undefined && !r.checkpoint || r.start !== undefined && !r.start)
            throw new Error('Invalid pet checkpoint.');
        for (const c of [r.start, r.checkpoint])
            if (c && (c.sim?.expressionVersion ?? 1) !== version)
                throw new Error('Pet expression version does not match its checkpoint.');
        if (r.petReplayVersion === 1 && (r.start || r.checkpoint && r.checkpoint.petCheckpointVersion !== 1))
            throw new Error('Invalid legacy pet recording.');
        if (r.petReplayVersion === 2 && (!r.start || r.start.petCheckpointVersion !== 2 || r.start.historyStart !== r.start.tick))
            throw new Error('The recording segment is missing its starting checkpoint.');
        const first = r.start && PetWorld.restore(points, r.tape, r.interactions, r.start);
        const world = r.checkpoint ? PetWorld.restore(points, r.tape, r.interactions, r.checkpoint) : first ?? new PetWorld(points, r.tape, r.interactions, version);
        if (first) {
            if (!world.segmented || world.tick < first.tick || r.checkpoint && r.checkpoint.historyStart !== first.tick)
                throw new Error('Pet segment checkpoints do not agree.');
            world.origin = first.checkpoint();
        }
        return world;
    }
    /** Retire only consumed input. The exact origin makes each archived segment
     * independently replayable; no particle, random stream or score is reset. */
    prepareSegment() {
        const dropTape = Math.max(0, this.bucketIndex), dropInputs = this.interactionIndex, previous = this.origin;
        const tape = this.tapeLog.slice(dropTape), interactions = this.interactionLog.slice(dropInputs);
        const points = this.sim.p.map(p => [p.hx, p.hy]);
        const c = this.checkpoint();
        c.petCheckpointVersion = 2;
        c.historyStart = this.tick;
        c.hasTelemetry = this.hasTelemetry;
        c.bucketIndex -= dropTape;
        c.interactionIndex = 0;
        c.history = new PetWorld(points, tape, interactions, this.sim.expressionVersion, true).historyDigest();
        const next = PetWorld.restore(points, tape, interactions, c);
        next.origin = next.checkpoint();
        let committed = false;
        return { recording: next.recording(), archive: this.recording(true, true), commit: () => {
                if (committed || this.origin !== previous || this.tick < c.tick)
                    throw new Error('The pet recording segment has changed.');
                this.tapeLog.splice(0, dropTape);
                this.interactionLog.splice(0, dropInputs);
                this.bucketIndex -= dropTape;
                this.interactionIndex -= dropInputs;
                this.tapeHashes = [];
                this.hashTape(0);
                this.segmented = true;
                this.origin = next.origin;
                committed = true;
            } };
    }
    /** Lossless version 1 export, including the current pose and score. Drivers
     * consume all chunks synchronously on the world's owner before another tick.
     * Only a small slice is serialized inside an embedded runtime at a time. */
    recordingChunk(index, completed = false) {
        if (!Number.isSafeInteger(index) || index < 0)
            throw new Error('Invalid pet export cursor.');
        const size = 16, tapeEnd = completed ? this.bucketIndex + 1 : this.tapeLog.length;
        const inputEnd = completed ? this.interactionIndex : this.interactionLog.length;
        const tapes = Math.ceil(tapeEnd / size), inputs = Math.ceil(inputEnd / size);
        if (index === 0)
            return `{"petReplayVersion":${this.segmented ? 2 : 1},"expressionVersion":${this.sim.expressionVersion},"tape":[`;
        if (index <= tapes)
            return (index === 1 ? '' : ',') + JSON.stringify(this.tapeLog.slice((index - 1) * size, Math.min(tapeEnd, index * size))).slice(1, -1);
        if (index === tapes + 1)
            return '],"interactions":[';
        const part = index - tapes - 2;
        if (part < inputs)
            return (part === 0 ? '' : ',') + JSON.stringify(this.interactionLog.slice(part * size, Math.min(inputEnd, (part + 1) * size))).slice(1, -1);
        if (part === inputs) {
            const history = this.historyDigest(tapeEnd, inputEnd);
            return `]${this.origin ? ',"start":' + JSON.stringify({ ...this.origin, hasTelemetry: this.hasTelemetry, history }) : ''},"checkpoint":${JSON.stringify({ ...this.checkpoint(), history })}}`;
        }
        return null;
    }
    /** Prefix states preserve the original FNV checksum byte for byte. Live
     * appends and replacements hash only the changed suffix, so checkpointing
     * does not rescan hours of accepted telemetry on the world worker. */
    hashTape(from) {
        for (let i = from; i < this.tapeLog.length; i++)
            this.tapeHashes[i] = (0, model_js_1.stableHash)((i ? ',' : '') + JSON.stringify(this.tapeLog[i]), this.tapeHashes[i - 1] ?? (0, model_js_1.stableHash)('[['));
    }
    historyDigest(tapeEnd = this.tapeLog.length, inputEnd = this.interactionLog.length) {
        let hash = (0, model_js_1.stableHash)('],[', this.tapeHashes[tapeEnd - 1] ?? (0, model_js_1.stableHash)('[['));
        for (let i = 0; i < inputEnd; i++)
            hash = (0, model_js_1.stableHash)((i ? ',' : '') + JSON.stringify(this.interactionLog[i]), hash);
        return (0, model_js_1.stableHash)(']]', hash);
    }
    /** Hydrate a new world without stepping history. Validation finishes before
     * the caller receives it, so a corrupt checkpoint never mutates a live pet. */
    static restore(points, tape, interactions, value) {
        const c = value;
        const range = (n, low, high) => Number.isFinite(n) && n >= low && n <= high;
        const integer = (n, low, high) => Number.isSafeInteger(n) && range(n, low, high);
        if (!c || ![1, 2].includes(c.petCheckpointVersion) || JSON.stringify(c).length > 512 * 1024
            || c.petCheckpointVersion === 2 && (!integer(c.historyStart, 0, c.tick) || typeof c.hasTelemetry !== 'boolean')
            || !integer(c.tick, 0, pet_sim_js_1.PET_MAX_SECONDS * HZ) || !range(c.accumulator, -1e-8, 1 / HZ + 1e-8)
            || !integer(c.bucketIndex, -1, tape.length - 1) || !integer(c.interactionIndex, 0, interactions.length)
            || !integer(c.branchTick, -1, c.tick) || !integer(c.random, 0, 0xffffffff)
            || !['swim', 'dive', 'roll', 'breathe', 'drift', 'doze', 'wake'].includes(c.behaviour)
            || !range(c.until, 0, pet_sim_js_1.PET_MAX_SECONDS + 10) || ![c.targetX, c.targetY, c.x, c.y, c.flip].every(n => range(n, -1, 1))
            || !range(c.lit, 0, 1) || !range(c.lastActivity, 0, c.tick / HZ)
            || c.addressedAt !== null && !range(c.addressedAt, 0, c.tick / HZ)
            || !range(c.waitSince, -1, c.tick / HZ) || typeof c.lastStill !== 'string' || c.lastStill.length > 2048
            || !Array.isArray(c.members) || c.members.length > 6
            || c.members.some(m => !m || typeof m.id !== 'string' || !m.id || m.id.length > 4096
                || !integer(m.slot, 0, 5) || !range(m.phase, 0, Math.PI * 2) || typeof m.present !== 'boolean')
            || new Set(c.members.map(m => m.id)).size !== c.members.length || new Set(c.members.map(m => m.slot)).size !== c.members.length
            || c.food !== null && (!c.food || !range(c.food.time, 0, c.tick / HZ) || ![c.food.x, c.food.y].every(n => range(n, -1, 1)))
            || !c.frame || c.frame.timeMs !== c.tick * 1000 / HZ || c.frame.behaviour !== c.behaviour
            || !['none', 'orient', 'approach', 'call'].includes(c.frame.needs)
            || !range(c.frame.surface, -.9, -.8) || !range(c.frame.caustic, 0, 1)
            || c.frame.food !== null && (!c.frame.food || !range(c.frame.food.x, -1, 1) || !range(c.frame.food.y, -1, 1.21) || !range(c.frame.food.life, 0, 1))
            || JSON.stringify(c.frame.pod) !== JSON.stringify(c.members)
            || !Array.isArray(c.voices) || c.voices.length > 1024
            || c.voices.some(v => !v || typeof v.id !== 'string' || v.id.length > 256))
            throw new Error('Invalid pet world checkpoint.');
        (0, pet_sim_js_1.validatePetState)(c.frame.state);
        (0, pet_audio_js_1.renderPetPCM)(c.voices, 0, 0);
        const world = new PetWorld(points, tape, interactions, c.sim?.expressionVersion ?? 1, c.petCheckpointVersion === 2);
        if (c.petCheckpointVersion === 2) {
            world.hasTelemetry = c.hasTelemetry;
            world.origin = structuredClone(c);
        }
        if (c.history !== world.historyDigest()
            || c.bucketIndex >= 0 && world.tape[c.bucketIndex].simTimeMs > c.frame.timeMs + 1e-7
            // acceptTelemetry may fill past gaps after the most recent fixed tick.
            // Preserve that pending cursor exactly; it may lag only over empty gaps.
            || world.tape.some((b, i) => i > c.bucketIndex && b.simTimeMs <= c.frame.timeMs + 1e-7
                && (b.observed !== 0 || b.channel !== 'other' || b.errors || b.waiting || b.agentIds.length
                    || b.onsets.some(Boolean) || b.activeMs.some(Boolean)))
            || world.interactionLog.slice(0, c.interactionIndex).some(e => e.timeMs > c.frame.timeMs + 1e-7)
            || world.interactionLog[c.interactionIndex]?.timeMs <= c.frame.timeMs + 1e-7)
            throw new Error('Pet checkpoint does not match its recording.');
        const candidate = world.tape[c.bucketIndex];
        const telemetry = candidate && c.frame.timeMs < candidate.simTimeMs + candidate.durationMs ? candidate : undefined;
        if (JSON.stringify(c.frame.telemetry) !== JSON.stringify(telemetry))
            throw new Error('Pet checkpoint telemetry does not match its clock.');
        world.sim.restore(c.sim);
        world.score.restore(c.score);
        world.random.restore(c.random);
        world.accumulator = c.accumulator;
        world.tick = c.tick;
        world.bucketIndex = c.bucketIndex;
        world.interactionIndex = c.interactionIndex;
        world.branchTick = c.branchTick;
        world.behaviour = c.behaviour;
        world.until = c.until;
        world.targetX = c.targetX;
        world.targetY = c.targetY;
        world.x = c.x;
        world.y = c.y;
        world.flip = c.flip;
        world.lit = c.lit;
        world.lastActivity = c.lastActivity;
        world.addressedAt = c.addressedAt ?? -Infinity;
        world.waitSince = c.waitSince;
        world.food = structuredClone(c.food);
        world.members = new Map(c.members.map(m => [m.id, { ...m }]));
        world.lastStill = c.lastStill;
        // JSON omits undefined properties; keep the same frame shape as makeFrame.
        world.frame = { ...structuredClone(c.frame), telemetry: telemetry ? structuredClone(telemetry) : undefined };
        world.voices = structuredClone(c.voices);
        return world;
    }
    /** Resume observation beyond all already accepted live packets, without
     * replaying their sound or exposing a stale request as current work. */
    resumeObservation() {
        const last = this.tapeLog.at(-1), end = last ? (last.sequence + 1) * 12 : 0;
        if (!end || end - this.tick > 24)
            throw new Error('Only a live recording can resume observation.');
        while (this.tick < end)
            this.step(1 / 30, { motion: false, sensitivity: 1 });
        this.voices = [];
    }
    /** Branch at the current playhead; input is journalled for the next fixed tick.
     * A live touch never needs to re-simulate the creature's entire lifetime. */
    interact(kind, x, y) {
        if (!['attention', 'food'].includes(kind) || !Number.isFinite(x) || !Number.isFinite(y) || Math.abs(x) > 1 || Math.abs(y) > 1)
            throw new Error('Invalid pet interaction.');
        if (this.branchTick !== this.tick)
            this.interactionLog.splice(this.interactionIndex);
        this.branchTick = this.tick;
        this.interactionLog.push({ timeMs: (this.tick + 1) * 1000 / HZ, kind, x, y });
    }
    /** Accept a live source packet at the next 400ms boundary. The accepted tape,
     * including any missing intervals, is the exact replay authority for this host. */
    acceptTelemetry(input) {
        (0, pet_telemetry_js_1.validatePetBucket)(input);
        const sequence = Math.floor(this.tick / 12) + 1;
        if (this.tapeLog.length >= 216_000)
            throw new Error('Archive this pet recording before accepting more telemetry.');
        this.hasTelemetry = true;
        if (this.segmented) {
            const at = this.tapeLog.findIndex(b => b.sequence >= sequence);
            const index = at < 0 ? this.tapeLog.length : at;
            this.tapeLog.splice(index, at >= 0 && this.tapeLog[at].sequence === sequence ? 1 : 0, { ...structuredClone(input), sequence, simTimeMs: sequence * 400 });
            this.hashTape(index);
            return;
        }
        if (sequence >= 216_000)
            throw new Error('Archive the legacy recording before accepting more telemetry.');
        const changedFrom = Math.min(sequence, this.tapeLog.length);
        while (this.tapeLog.length <= sequence) {
            const at = this.tapeLog.length;
            this.tapeLog.push({ version: 1, sequence: at, simTimeMs: at * 400, durationMs: 400,
                activity: .12, coherence: .25, attention: 0, channel: 'other', observed: 0, roamX: 0, roamY: 0, flip: 1, lit: 1,
                onsets: Array(13).fill(0), activeMs: Array(13).fill(0), errors: 0, agentIds: [], waiting: false });
        }
        this.tapeLog[sequence] = { ...structuredClone(input), sequence, simTimeMs: sequence * 400 };
        this.hashTape(changedFrom);
    }
    /** dt is bounded so suspending a surface cannot cause an unbounded catch-up. */
    step(dt, opts = { motion: true, sensitivity: 1 }) {
        if (!Number.isFinite(dt) || dt < 0 || dt > 10)
            throw new Error('World dt must be in [0, 10] seconds.');
        this.accumulator += dt;
        this.voices = [];
        while (this.accumulator + 1e-10 >= 1 / HZ) {
            this.accumulator -= 1 / HZ;
            const time = ++this.tick / HZ;
            this.frame = this.makeFrame(time);
            this.voices.push(...this.score.voices(this.frame));
            if (!opts.motion) {
                this.frame.state = { ...this.frame.state, roamX: 0, roamY: 0, flip: 1, lit: this.behaviour === 'doze' ? .18 : 1 };
                this.frame.surface = -.86;
                this.frame.caustic = .5;
                if (this.frame.food)
                    this.frame.food.y = this.food.y;
            }
            const signature = JSON.stringify(this.frame.state);
            const peers = this.frame.pod.filter(p => p.present);
            const podSlots = peers.length >= 3 ? peers.map(p => [[0, 2, 4, 1, 3, 5][p.slot], p.phase]) : undefined;
            if (opts.motion || signature !== this.lastStill || podSlots)
                this.sim.step(1 / HZ, this.frame.state, { ...opts, podSlots });
            this.lastStill = signature;
        }
        return this.frame;
    }
    makeFrame(time) {
        const timeMs = this.tick * 1000 / HZ;
        while (this.bucketIndex + 1 < this.tape.length && this.tape[this.bucketIndex + 1].simTimeMs <= timeMs + 1e-7)
            this.bucketIndex++;
        const candidate = this.tape[this.bucketIndex];
        const telemetry = candidate && timeMs < candidate.simTimeMs + candidate.durationMs ? candidate : undefined;
        while (this.interactionIndex < this.interactionLog.length && this.interactionLog[this.interactionIndex].timeMs <= timeMs + 1e-7) {
            const e = this.interactionLog[this.interactionIndex++];
            this.addressedAt = time;
            this.lastActivity = time;
            this.targetX = e.x * .65;
            this.targetY = e.y * .65;
            if (e.kind === 'food')
                this.food = { time, x: e.x, y: e.y };
        }
        const busy = telemetry && telemetry.observed >= .92 && telemetry.activity > .2;
        if (busy)
            this.lastActivity = time;
        if (telemetry?.waiting) {
            if (this.waitSince < 0)
                this.waitSince = time;
        }
        else
            this.waitSince = -1;
        const waitingFor = this.waitSince < 0 ? 0 : time - this.waitSince;
        const needs = this.waitSince < 0 ? 'none' : waitingFor < 8 ? 'orient' : waitingFor < 25 ? 'approach' : 'call';
        const addressed = time - this.addressedAt < 2;
        if (this.behaviour === 'doze' && (busy || addressed)) {
            this.behaviour = 'wake';
            this.until = time + 2;
        }
        else if (!busy && !addressed && time - this.lastActivity >= 75)
            this.behaviour = 'doze';
        else if (time >= this.until) {
            const choices = ['swim', 'swim', 'dive', 'roll', 'breathe', 'drift'];
            this.behaviour = choices[Math.floor(this.random() * choices.length)];
            this.until = time + 3 + this.random() * 7;
            this.targetX = (this.random() * 2 - 1) * .75;
            this.targetY = this.behaviour === 'dive' ? .6 : this.behaviour === 'breathe' ? -.65 : (this.random() * 2 - 1) * .4;
        }
        const sleeping = this.behaviour === 'doze';
        if (sleeping) {
            this.targetY = .65;
            this.targetX = .15;
        }
        if (this.sim.expressionVersion === 1 && (needs === 'approach' || needs === 'call')) {
            this.targetX = 0;
            this.targetY = .3;
        }
        const move = sleeping ? .004 : this.behaviour === 'drift' ? .006 : .018;
        const dx = this.targetX - this.x;
        this.x += dx * move;
        this.y += (this.targetY - this.y) * move;
        this.flip += ((Math.abs(dx) < .015 ? this.flip < 0 ? -1 : 1 : dx < 0 ? -1 : 1) - this.flip) * .035;
        this.lit += ((sleeping ? .18 : 1) - this.lit) * .03;
        const wild = !this.hasTelemetry;
        const state = {
            activity: telemetry?.activity ?? (wild ? sleeping ? .05 : .18 : .12),
            coherence: telemetry?.coherence ?? (wild ? .94 : .25),
            attention: Math.max(telemetry?.attention ?? 0, addressed ? .85 : 0, this.sim.expressionVersion === 1 && needs === 'call' ? 1 : 0),
            // A touch changes orientation, never hides an instrumentation gap.
            channel: addressed ? 'human' : telemetry?.channel ?? 'other',
            observed: telemetry?.observed ?? (wild ? 1 : 0),
            roamX: this.x, roamY: this.y, flip: this.flip, lit: this.lit,
        };
        const activeIds = new Set(telemetry?.agentIds ?? []);
        for (const member of this.members.values())
            member.present = activeIds.has(member.id);
        for (const id of activeIds) {
            if (!this.members.has(id)) {
                const vacant = [...this.members.values()].find(m => !m.present);
                const slot = this.members.size < 6 ? this.members.size : vacant?.slot;
                if (slot !== undefined) {
                    if (this.members.size >= 6 && vacant)
                        this.members.delete(vacant.id);
                    this.members.set(id, { id, slot, phase: (0, pet_sim_js_1.mulberry32)(seedFor(`pod:${id}`))() * Math.PI * 2, present: true });
                }
            }
            const member = this.members.get(id);
            if (member)
                member.present = true;
        }
        return { timeMs, behaviour: this.behaviour, state, telemetry, needs,
            pod: [...this.members.values()].map(m => ({ ...m })),
            surface: -.86 + .012 * Math.sin(time * .8), caustic: .5 + .5 * Math.sin(time * .31),
            food: this.food && time - this.food.time < 5
                ? { x: this.food.x, y: this.food.y + (time - this.food.time) * .04, life: (0, model_js_1.clamp)(1 - (time - this.food.time) / 5, 0, 1) } : null };
    }
}
exports.PetWorld = PetWorld;

};
factories["model"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.durationOf = exports.EMPTY_FILTERS = exports.CATEGORIES = void 0;
exports.eventMatches = eventMatches;
exports.errorOnsetOf = errorOnsetOf;
exports.stableHash = stableHash;
exports.quantile = quantile;
exports.clamp = clamp;
exports.formatTime = formatTime;
/** Versioned, vendor-neutral trace and signal contracts. All internal time is ms. */
exports.CATEGORIES = [
    'reasoning', 'tool', 'memory', 'code', 'filesystem', 'network', 'browser',
    'communication', 'agent', 'orchestration', 'error', 'human', 'other',
];
exports.EMPTY_FILTERS = { query: '', category: '', agent: '', model: '', tool: '', status: '' };
function eventMatches(e, f) {
    if (f.category && e.category !== f.category)
        return false;
    if (f.agent && e.agentId !== f.agent)
        return false;
    if (f.model && e.model !== f.model)
        return false;
    if (f.tool && e.tool !== f.tool)
        return false;
    if (f.status && e.status !== f.status)
        return false;
    if (f.query) {
        const q = f.query.toLowerCase();
        // Search is deliberately content-aware but runs only over locally retained fields.
        if (![e.name, e.id, e.agentId, e.model, e.tool, e.provider, e.category,
            JSON.stringify(e.attributes), JSON.stringify(e.payload), JSON.stringify(e.raw)].filter(Boolean).join(' ').toLowerCase().includes(q))
            return false;
    }
    return true;
}
const durationOf = (e) => Math.max(0, e.endTime - e.startTime);
exports.durationOf = durationOf;
/** An explicit failure receipt can arrive after a span began or ended. Keep
 * its timestamp distinct from the operation onset in every signal view. */
function errorOnsetOf(e) {
    const time = e.attributes['whalesong.error_onset_ms'];
    if (time === undefined)
        return e.startTime;
    if (typeof time !== 'number' || !Number.isFinite(time) || time < e.startTime)
        throw new Error('Invalid failure observation time.');
    return time;
}
function stableHash(text, seed = 2166136261) {
    let h = seed;
    for (let i = 0; i < text.length; i++) {
        h ^= text.charCodeAt(i);
        h = Math.imul(h, 16777619);
    }
    return h >>> 0;
}
function quantile(a, q) {
    if (!a.length)
        return 0;
    const s = [...a].sort((a, b) => a - b), x = Math.min(1, Math.max(0, q)) * (s.length - 1);
    return s[Math.floor(x)] + (s[Math.ceil(x)] - s[Math.floor(x)]) * (x % 1);
}
function clamp(x, lo, hi) { return Math.max(lo, Math.min(hi, x)); }
function formatTime(ms, precise = false) {
    const m = Math.floor(Math.max(0, ms) / 60000), s = Math.floor(Math.max(0, ms) / 1000) % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}${precise ? '.' + String(Math.floor(ms % 1000)).padStart(3, '0') : ''}`;
}

};
factories["pet-sim"]=function(exports,require){
"use strict";
// PetSim — the portable core of the Codewhale pet.
//
// This is a faithful, DOM-free port of the consort study's particle engine
// (grammar.js). The same code — same constants, same order of operations — is
// what the Rust, Swift and Kotlin cores implement. Four rules keep the view
// identical on every surface:
//
//   1. The body is the same 980 points, loaded from whale-points.tsv — never
//      re-sampled from the image on each platform.
//   2. Per-particle jitter phases come from mulberry32(0xC0FFEE), not from the
//      platform's RNG.
//   3. All motion is a pure function of (sim clock, state): nothing accumulates
//      noise. Two runs fed the same tape produce the same frame.
//   4. Colour, hollowness and brightness are computed here, once — renderers
//      only place and stamp dots.
//
// Positions stay normalized in body space (roughly [-0.5, 0.5]²). A renderer
// maps them through layout() to its own medium.
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetSim = exports.CHANNEL_INDEX = exports.CHANNELS = exports.ARCH_OF = exports.REST_STATE = exports.PET_MAX_SECONDS = void 0;
exports.validatePetState = validatePetState;
exports.mulberry32 = mulberry32;
exports.layout = layout;
exports.digest = digest;
exports.runTape = runTape;
exports.PET_MAX_SECONDS = 100 * 365 * 86_400;
exports.REST_STATE = {
    activity: 0.35, coherence: 0.8, attention: 0, channel: 'reasoning',
    observed: 1, roamX: 0, roamY: 0, flip: 1, lit: 1,
};
const lerp = (a, b, t) => a + (b - a) * t;
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
exports.ARCH_OF = {
    reasoning: 'gyre', memory: 'gyre',
    tool: 'strike', code: 'strike', filesystem: 'strike',
    network: 'cross', communication: 'cross', browser: 'cross',
    agent: 'pod', orchestration: 'pod',
    error: 'tear', human: 'address', other: 'drift',
};
exports.CHANNELS = [
    { key: 'reasoning', label: 'Model / reasoning', color: '#73c9b5', freq: 130.81, sustained: true, arch: 'gyre', form: 'gyre · rolling' },
    { key: 'tool', label: 'Tool calls', color: '#74aadd', freq: 261.63, sustained: false, arch: 'strike', form: 'strike · reaching' },
    { key: 'memory', label: 'Memory / RAG', color: '#b6a77f', freq: 195.99, sustained: true, arch: 'gyre', form: 'gyre · scanning' },
    { key: 'code', label: 'Code execution', color: '#9b9ed7', freq: 164.81, sustained: false, arch: 'strike', form: 'strike · along the body' },
    { key: 'filesystem', label: 'Filesystem', color: '#92b9c9', freq: 440.00, sustained: false, arch: 'strike', form: 'strike · fanning' },
    { key: 'network', label: 'Network / API', color: '#d3ac74', freq: 523.25, sustained: false, arch: 'cross', form: 'crossing · one way' },
    { key: 'browser', label: 'Browser / computer', color: '#9ea9df', freq: 349.23, sustained: false, arch: 'cross', form: 'crossing · a sweep' },
    { key: 'communication', label: 'Agent messages', color: '#83c5c9', freq: 293.66, sustained: false, arch: 'cross', form: 'crossing · two ways' },
    { key: 'agent', label: 'Subagent activity', color: '#b09acb', freq: 220.00, sustained: true, arch: 'pod', form: 'pod · peers' },
    { key: 'orchestration', label: 'Orchestration', color: '#6c8798', freq: 98.00, sustained: true, arch: 'pod', form: 'pod · hub' },
    { key: 'error', label: 'Errors / exceptions', color: '#e79186', freq: 185.00, sustained: false, arch: 'tear', form: 'torn · irregular' },
    { key: 'human', label: 'Human interaction', color: '#c2b787', freq: 391.99, sustained: false, arch: 'address', form: 'decision · junction' },
    { key: 'other', label: 'Unclassified', color: '#738492', freq: 146.83, sustained: false, arch: 'drift', form: 'drifting · unformed' },
];
exports.CHANNEL_INDEX = Object.fromEntries(exports.CHANNELS.map((c, i) => [c.key, i]));
function validatePetState(value) {
    const s = value;
    if (!s || typeof s !== 'object' || !Object.hasOwn(exports.CHANNEL_INDEX, s.channel)
        || ![s.activity, s.coherence, s.attention, s.observed, s.lit].every(n => Number.isFinite(n) && n >= 0 && n <= 1)
        || ![s.roamX, s.roamY, s.flip].every(n => Number.isFinite(n) && Math.abs(n) <= 1))
        throw new Error('Invalid pet state.');
}
const hex2rgb = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const RGB = exports.CHANNELS.map(c => hex2rgb(c.color));
const UNKNOWN_RGB = hex2rgb('#738492');
const REST_RGB = [122, 214, 240];
// mulberry32 — a 32-bit seeded PRNG tiny enough to port by hand correctly.
function mulberry32(seed) {
    let a = seed >>> 0;
    return Object.assign(() => {
        a = (a + 0x6D2B79F5) >>> 0;
        let t = a;
        t = Math.imul(t ^ (t >>> 15), t | 1);
        t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    }, { state: () => a, restore: (state) => {
            if (!Number.isSafeInteger(state) || state < 0 || state > 0xffffffff)
                throw new Error('Invalid pet random stream.');
            a = state;
        } });
}
/** Work reorganizes the same particles; no new random draws or invented facts.
 * These are expressive fields, not diagrams of unobserved network/file topology. */
function fieldTarget(q, t, act, att, key) {
    const u = q.s * 2 - 1, lane = q.pod - 2.5, a = q.s * Math.PI * 2;
    const flow = t * (.35 + act * .65);
    if (key === 'reasoning') {
        const ring = .34 + .105 * Math.cos(a * 3 + flow + lane * .18);
        return [ring * Math.cos(a * 2 + flow * .3), ring * Math.sin(a * 2 + flow * .3) * .7 + .10 * Math.sin(a * 3 + flow)];
    }
    if (key === 'memory')
        return [.46 * Math.cos(a + lane * .1 + flow * .25), lane * .082 + .052 * Math.sin(a * 2 + flow)];
    if (key === 'code')
        return [u * .57, lane * .066 + .12 * Math.sin(u * 7 + flow * 2 + q.pod * Math.PI / 3)];
    if (key === 'filesystem') {
        const branch = Math.max(0, (u + .3) / 1.3);
        return [u * .56, lane * .13 * branch + .025 * Math.sin(u * 8 - flow)];
    }
    if (key === 'tool') {
        const reach = .14 + (u + 1) * .20 + .04 * Math.sin(flow * 3 - u * 4);
        return [Math.cos(q.pod * Math.PI / 3) * reach, Math.sin(q.pod * Math.PI / 3) * reach * .8 + q.hy * .06];
    }
    if (key === 'browser')
        return [u * .56, lane * .083 + .035 * Math.sin(u * 5 - flow * 2)];
    if (key === 'network' || key === 'communication') {
        const direction = key === 'communication' && q.pod % 2 === 1 ? -1 : 1;
        const phase = a + flow * direction;
        return [.54 * Math.cos(phase), Math.sin(phase) * (.12 + q.pod * .035) + lane * .024];
    }
    if (key === 'human') {
        const gap = u < 0 ? -.075 : .075;
        return [u * .47 + gap, lane * .10 * Math.abs(u) + .012 * Math.sin(flow + a) * (1 - att)];
    }
    return undefined;
}
// Version 1 retains the original authored gait for existing recordings.
function gaitTarget(q, t, act, coh, att, key, work, podSlots, expressionVersion = 1) {
    const { hx, hy, ang, rad, tail, s, pod, jx, jy } = q;
    const omega = lerp(4.6, 5.2 + act * 2.8, work);
    const breath = 1 + Math.sin(t * 1.85) * lerp(0.048, 0.018, work);
    const flex = Math.sin(ang * 2.05 + t * omega) * lerp(0.042, 0.016 + act * 0.028, work) * (0.18 + 0.82 * tail);
    let px = Math.cos(ang + flex) * rad * breath;
    let py = Math.sin(ang + flex) * rad * breath;
    px += Math.sin(t * 0.33) * lerp(0.030, 0.014, work);
    py += Math.cos(t * 0.21) * lerp(0.018, 0.010, work);
    if (work < 0.02)
        return [px, py];
    const arch = exports.ARCH_OF[key] || 'drift';
    let gx = px, gy = py;
    if (arch === 'gyre') {
        if (key === 'memory') {
            const pulse = 1 + Math.sin(t * (2.4 + act * 1.6) - rad * 11) * (0.15 + act * 0.10);
            gx *= pulse;
            gy *= pulse;
        }
        else {
            const roll = Math.sin(t * (1.05 + act * 0.35)) * (0.48 + act * 0.32);
            const c = Math.cos(roll), sn = Math.sin(roll);
            gx = px * c - py * sn * 0.88;
            gy = px * sn * 0.88 + py * c;
        }
    }
    else if (arch === 'strike') {
        if (key === 'tool') {
            const rate = 2.7 + act * 2.1;
            const lunge = Math.pow(Math.max(0, Math.sin(t * rate)), 2);
            gx += lunge * 0.11;
            if (s > 0.60) {
                const reach = Math.pow(Math.max(0, Math.sin(t * rate + pod * 0.92)), 4) * (0.30 + act * 0.24);
                gx += Math.cos(ang) * reach;
                gy += Math.sin(ang) * reach;
            }
        }
        else if (key === 'code') {
            const rate = 3.2 + act * 1.8;
            const wave = Math.sin(t * rate - tail * 7.5);
            const bump = 0.11 + act * 0.08;
            gx += Math.cos(ang) * wave * bump;
            gy += Math.sin(ang) * wave * bump * 1.2;
            gx += Math.max(0, wave) * 0.07;
        }
        else {
            const rate = 2.15 + act * 1.5;
            const side = (pod % 2) * 2 - 1;
            const w = Math.pow(Math.max(0, Math.sin(t * rate + pod * 0.72)), 2);
            gx += w * 0.055;
            gy += side * w * (0.17 + act * 0.13);
        }
    }
    else if (arch === 'cross') {
        if (key === 'browser') {
            const band = ((t * (0.55 + act * 0.35)) % 1) * 1.28 - 0.64;
            const inBand = Math.max(0, 1 - Math.abs(hy - band) / 0.08);
            gx += inBand * (0.24 + act * 0.10);
            gy += inBand * 0.02;
        }
        else {
            const two = key === 'communication';
            const courier = s < (two ? 0.44 : 0.32);
            if (courier) {
                const dir = two ? (s < 0.22 ? 1 : -1) : 1;
                const u = (t * (0.38 + act * 0.36) + s * 5.2) % 1;
                const going = u < 0.5 ? u * 2 : 2 - u * 2;
                const e = going * going * (3 - 2 * going);
                gx = lerp(hx, dir * 0.80, e);
                gy = hy * (1 - e * 0.38) + Math.sin(going * Math.PI) * 0.11 * dir;
            }
        }
    }
    else if (arch === 'pod') {
        const n = 6, member = podSlots?.length ? podSlots[pod % podSlots.length] : undefined;
        const k = member ? member[0] : pod % n;
        const hub = key === 'orchestration' && k === 0;
        const spread = 0.30 + act * 0.11;
        const orbit = t * (0.55 + act * 0.28);
        if (hub) {
            gx = px * 0.70;
            gy = py * 0.70;
        }
        else {
            const slots = key === 'orchestration' ? n - 1 : n;
            const a = (key === 'orchestration' ? k - 1 : k) * (Math.PI * 2 / slots) + orbit + (member ? member[1] * .04 : 0);
            const sc = 0.34;
            gx = hx * sc + Math.cos(a) * spread * 1.28;
            gy = hy * sc + Math.sin(a) * spread * 0.80;
        }
    }
    else if (arch === 'tear') {
        const side = hx + hy < 0 ? -1 : 1;
        gx += side * (0.24 + (1 - coh) * 0.16);
        gy += side * 0.15;
        gx += Math.sin(t * 11.4 + s * 40) * (0.045 + act * 0.05);
        gy += Math.cos(t * 9.2 + s * 31) * (0.040 + act * 0.045);
    }
    else if (arch === 'address') {
        const face = 0.90 + att * 0.08;
        const th = 0.70;
        const z = (s - 0.5) * 0.42;
        let ax = hx * Math.cos(th) + z * Math.sin(th);
        let ay = hy;
        const disc = 0.48 * face;
        ax = lerp(ax, Math.cos(ang) * Math.min(0.36, rad + 0.06) * 0.95, disc);
        ay = lerp(ay, Math.sin(ang) * Math.min(0.36, rad + 0.06) * 1.08, disc);
        const grow = 1.20 + Math.sin(t * 1.65) * 0.055;
        gx = ax * grow;
        gy = ay * grow;
    }
    else {
        const mill = 0.13 + (1 - coh) * 0.10;
        gx = hx * 0.52 + Math.sin(t * 0.72 + jx) * mill;
        gy = hy * 0.52 + Math.cos(t * 0.54 + jy) * mill;
    }
    if (expressionVersion === 2) {
        const field = fieldTarget(q, t, act, att, key);
        if (field)
            [gx, gy] = field;
    }
    return [lerp(px, gx, work), lerp(py, gy, work)];
}
// Fixed reduced-motion clock per channel, so ticks still point along the gait.
const STILL_T = {
    reasoning: 1.15, memory: 0.42, tool: 0.30, code: 0.18, filesystem: 0.48,
    network: 0.72, browser: 0.95, communication: 0.58, agent: 1.25,
    orchestration: 0.85, error: 0.35, human: 0.05, other: 0.90,
};
class PetSim {
    expressionVersion;
    p;
    phase = 0;
    clock = 0;
    tear = 0;
    prev;
    col = [...REST_RGB];
    cur;
    frame = { r: REST_RGB[0], g: REST_RGB[1], b: REST_RGB[2], alpha: 0.3, hollow: false, channel: 'reasoning', arch: 'gyre', work: 0 };
    constructor(points, seed = 0xC0FFEE, expressionVersion = 2) {
        this.expressionVersion = expressionVersion;
        if (expressionVersion !== 1 && expressionVersion !== 2)
            throw new Error('Unsupported pet expression version.');
        const rand = mulberry32(seed);
        this.p = points.map(([hx, hy], i) => {
            const q = {
                x: hx, y: hy, vx: 0, vy: 0,
                s: rand(), jx: rand() * 6.283, jy: rand() * 6.283, pod: i % 6,
                hx, hy, ang: 0, rad: 0, tail: 0, tx: hx, ty: hy,
            };
            q.ang = Math.atan2(hy, hx);
            q.rad = Math.hypot(hx, hy);
            q.tail = clamp(((-hx - hy) * 0.5 + 0.22) / 0.62);
            return q;
        });
        this.cur = this.prev = exports.CHANNEL_INDEX['reasoning'];
    }
    checkpoint() {
        return { version: 1, expressionVersion: this.expressionVersion, body: this.p.map(p => [p.hx, p.hy, p.s]),
            particles: this.p.map(p => [p.x, p.y, p.vx, p.vy, p.jx, p.jy, p.tx, p.ty]),
            phase: this.phase, clock: this.clock, tear: this.tear, previous: this.prev, current: this.cur,
            color: [...this.col], frame: { ...this.frame } };
    }
    /** Restore into a newly constructed sim. Authored body and seeded particle
     * identity must match exactly; a checkpoint cannot replace the whale. */
    restore(value) {
        const c = value;
        const inRange = (n, low, high) => Number.isFinite(n) && n >= low && n <= high;
        if (!c || c.version !== 1 || c.expressionVersion !== undefined && ![1, 2].includes(c.expressionVersion) || (c.expressionVersion ?? 1) !== this.expressionVersion || !Array.isArray(c.body) || c.body.length !== this.p.length
            || c.body.some((v, i) => !Array.isArray(v) || v.length !== 3 || v[0] !== this.p[i].hx || v[1] !== this.p[i].hy || v[2] !== this.p[i].s)
            || !Array.isArray(c.particles) || c.particles.length !== this.p.length
            || c.particles.some(v => !Array.isArray(v) || v.length !== 8 || v.some((n, i) => !inRange(n, i === 4 || i === 5 ? 0 : -8, i === 4 || i === 5 ? 2 * exports.PET_MAX_SECONDS : 8)))
            || !inRange(c.phase, 0, exports.PET_MAX_SECONDS) || !inRange(c.clock, 0, exports.PET_MAX_SECONDS) || !inRange(c.tear, 0, 1)
            || ![c.previous, c.current].every(n => Number.isInteger(n) && n >= 0 && n < exports.CHANNELS.length)
            || !Array.isArray(c.color) || c.color.length !== 3 || c.color.some(n => !inRange(n, 0, 255))
            || !c.frame || ![c.frame.r, c.frame.g, c.frame.b].every(n => inRange(n, 0, 255))
            || !inRange(c.frame.alpha, 0, 1) || !inRange(c.frame.work, 0, 1) || typeof c.frame.hollow !== 'boolean'
            || c.frame.channel !== exports.CHANNELS[c.current].key || c.frame.arch !== exports.CHANNELS[c.current].arch)
            throw new Error('Invalid pet particle checkpoint.');
        this.phase = c.phase;
        this.clock = c.clock;
        this.tear = c.tear;
        this.prev = c.previous;
        this.cur = c.current;
        this.col = [...c.color];
        this.frame = { ...c.frame };
        this.p.forEach((p, i) => { [p.x, p.y, p.vx, p.vy, p.jx, p.jy, p.tx, p.ty] = c.particles[i]; });
    }
    /** Advance the sim by dt seconds under `state`. Identical math on every port. */
    step(dt, state, opts) {
        const S = (v) => lerp(0.5, v, opts.sensitivity);
        const act = S(state.activity), coh = S(state.coherence), att = S(state.attention);
        const seen = S(state.observed === undefined ? 1 : state.observed);
        const motion = opts.motion ? 1 : 0;
        this.phase += dt * (0.18 + act * 0.55) * motion;
        this.clock += dt * (opts.motion ? 1 : 0);
        if (exports.CHANNEL_INDEX[state.channel] !== undefined)
            this.cur = exports.CHANNEL_INDEX[state.channel];
        const shown = this.cur;
        const ch = exports.CHANNELS[shown];
        const work = clamp((act - 0.16) / 0.18);
        const wander = lerp(0.32, 1, Math.pow(1 - coh, 1.15));
        if (shown !== this.prev) {
            if (shown === exports.CHANNEL_INDEX['error'])
                this.tear = 1;
            this.prev = shown;
        }
        this.tear = opts.motion ? Math.max(0, this.tear - dt * 1.6) : 0;
        const split = Math.pow(1 - coh, 1.6) * 0.16 + this.tear * 0.10;
        const blur = Math.pow(1 - coh, 1.45) * 0.22 + this.tear * 0.18;
        const pull = opts.motion ? (2.2 + coh * 5.2) : 18;
        const tGait = opts.motion ? this.clock : (STILL_T[ch.key] ?? 0.4);
        for (const q of this.p) {
            if (opts.motion) {
                q.jx += dt * (0.40 + act * 1.1);
                q.jy += dt * (0.34 + act * 0.9);
            }
            const [gx, gy] = gaitTarget(q, tGait, act, coh, att, ch.key, work, opts.podSlots, this.expressionVersion);
            const podAng = q.pod * 1.047 + this.phase * 0.22;
            const tx = gx + Math.sin(q.jx + q.s * 9) * blur * wander + Math.cos(podAng) * split;
            const ty = gy + Math.cos(q.jy + q.s * 7) * blur * wander + Math.sin(podAng) * split * 0.55;
            q.tx = tx;
            q.ty = ty;
            if (!opts.motion) {
                q.x = tx;
                q.y = ty;
                q.vx = 0;
                q.vy = 0;
                continue;
            }
            q.vx += (tx - q.x) * pull * dt;
            q.vy += (ty - q.y) * pull * dt;
            q.vx *= 0.90;
            q.vy *= 0.90;
            q.x += q.vx * dt * (opts.motion ? 2.6 : 8);
            q.y += q.vy * dt * (opts.motion ? 2.6 : 8);
        }
        // ---- visual encoding: the parts of "the same view" that are not motion
        const want = work > 0.35 ? RGB[shown] : REST_RGB;
        const k = opts.motion ? Math.min(1, dt * 2.6) : 1;
        for (let c = 0; c < 3; c++)
            this.col[c] += (lerp(UNKNOWN_RGB[c], want[c], seen) - this.col[c]) * k;
        const lit = clamp(state.lit);
        const alpha = (0.22 + act * 0.10) * lerp(0.50, 1, coh) * lerp(0.55, 1, seen) * lerp(0.35, 1, lit);
        this.frame = {
            r: this.col[0], g: this.col[1], b: this.col[2],
            alpha: Math.min(0.92, alpha * 1.85),
            hollow: seen < 0.92,
            channel: ch.key, arch: ch.arch, work,
        };
    }
}
exports.PetSim = PetSim;
/** Body-space → renderer-space. Renderers place each dot at (lx,ly) in pixels/cells. */
function layout(w, h, state) {
    const att = state.attention;
    const scale = Math.min(w * 0.52, h * 0.92) * (1 + att * 0.07);
    return {
        scale,
        flipX: state.flip,
        ox: w / 2 + state.roamX * w * 0.30,
        oy: h / 2 + state.roamY * h * 0.30 + h * att * 0.05,
        dot: Math.max(1.6, Math.min(w, h) * 0.0092) * (1 + att * 0.18),
    };
}
// ---------------------------------------------------------------------------
// Conformance. A tape is a list of [dt, state] rows; run it and digest the
// quantized field every `every` frames. 64×32 cells over [-0.66, 0.66]².
// Two implementations that produce the same digest lines drew the same whale.
function digest(sim) {
    const W = 64, H = 32;
    const grid = new Uint8Array(W * H);
    for (const q of sim.p) {
        const cx = Math.floor((q.x + 0.66) / 1.32 * W);
        const cy = Math.floor((q.y + 0.66) / 1.32 * H);
        if (cx >= 0 && cx < W && cy >= 0 && cy < H)
            grid[cy * W + cx] = Math.min(255, grid[cy * W + cx] + 1);
    }
    // FNV-1a 64 over the grid plus the frame encoding (rgb, hollow, alpha byte)
    // Two unsigned halves also work in embedded engines without BigInt.
    // FNV's prime is (256 << 32) + 435; these products stay below 2^42,
    // so every intermediate integer is exactly representable by a JS number.
    let hi = 0xcbf29ce4, lo = 0x84222325;
    const mix = (b) => {
        lo = (lo ^ (b & 0xff)) >>> 0;
        const product = lo * 435;
        hi = (hi * 435 + lo * 256 + Math.floor(product / 4294967296)) >>> 0;
        lo = product >>> 0;
    };
    for (const v of grid)
        mix(v);
    mix(Math.round(sim.frame.r));
    mix(Math.round(sim.frame.g));
    mix(Math.round(sim.frame.b));
    mix(Math.round(sim.frame.alpha * 255));
    mix(sim.frame.hollow ? 1 : 0);
    return hi.toString(16).padStart(8, '0') + lo.toString(16).padStart(8, '0');
}
/** Shared tape runner. `rows` are parsed tape.tsv lines. */
function runTape(sim, rows, opts) {
    const out = [];
    let f = 0;
    for (const row of rows) {
        const c = row.split('\t');
        if (c.length < 10 || c[0] === 'dt')
            continue;
        const st = {
            activity: +c[1], coherence: +c[2], attention: +c[3], channel: c[4],
            observed: +c[5], roamX: +c[6], roamY: +c[7], flip: +c[8], lit: +c[9],
        };
        sim.step(+c[0], st, opts);
        if (f++ % 30 === 0)
            out.push(`f${String(f - 1).padStart(4, '0')} ${digest(sim)} ${st.channel}`);
    }
    out.push(`final ${digest(sim)}`);
    return out;
}

};
factories["pet-telemetry"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetLiveTape = exports.PET_BIN_MS = void 0;
exports.validatePetBucket = validatePetBucket;
exports.decodePetJSONL = decodePetJSONL;
exports.compilePetTelemetry = compilePetTelemetry;
exports.encodePetJSONL = encodePetJSONL;
exports.encodePetTSV = encodePetTSV;
const model_js_1 = require("./model.js");
const signal_js_1 = require("./signal.js");
const pet_sim_js_1 = require("./pet-sim.js");
/** One projection for imports, demos and recorded live snapshots. Times are ms.
 * These are aesthetic encodings of measured events, never model confidence. */
exports.PET_BIN_MS = 400;
function validatePetBucket(value) {
    (0, pet_sim_js_1.validatePetState)(value);
    const b = value;
    if (!b || typeof b !== 'object' || b.version !== 1 || !Number.isSafeInteger(b.sequence) || b.sequence < 0 || b.sequence > pet_sim_js_1.PET_MAX_SECONDS * 2.5
        || b.simTimeMs !== b.sequence * exports.PET_BIN_MS || b.durationMs !== exports.PET_BIN_MS
        || !model_js_1.CATEGORIES.includes(b.channel) || typeof b.waiting !== 'boolean'
        || !Array.isArray(b.agentIds) || b.agentIds.length > 250_000 || b.agentIds.some(id => typeof id !== 'string' || !id || id.length > 4096)
        || !Number.isSafeInteger(b.errors) || b.errors < 0 || b.errors > 250_000
        || !Array.isArray(b.onsets) || b.onsets.length !== 13 || b.onsets.some(n => !Number.isSafeInteger(n) || n < 0 || n > 250_000)
        || !Array.isArray(b.activeMs) || b.activeMs.length !== 13 || b.activeMs.some(n => !Number.isFinite(n) || n < 0 || n > 100_000_000))
        throw new Error('Invalid version 1 pet bucket.');
}
function decodePetJSONL(text) {
    if (text.length > 64 * 1024 * 1024)
        throw new Error('Pet tape exceeds 64 MiB.');
    const rows = text.split(/\r?\n/).filter(line => line.trim()).map(line => JSON.parse(line));
    if (rows.length > 216_000)
        throw new Error('Pet tape exceeds 24 hours.');
    return rows.map((row, i) => { validatePetBucket(row); if (row.sequence !== i)
        throw new Error('Non-contiguous pet tape.'); return row; });
}
/** A live file must advance before its contents count as a new observation.
 * Existing bytes, duplicate samples and a restarted sequence establish a
 * baseline; they never replay an old onset or human request. Drivers supply a
 * bounded tail and reset this cursor after suspension or a new attachment. */
class PetLiveTape {
    sequence;
    reset() { this.sequence = undefined; }
    readTail(text) {
        if (!text) {
            this.reset();
            return;
        }
        if (text.length > 262_144) {
            this.reset();
            throw new Error('Live pet input exceeds its tail limit.');
        }
        if (!text.endsWith('\n'))
            return;
        const line = text.trimEnd().split('\n').at(-1);
        if (!line)
            return;
        let packet;
        try {
            packet = JSON.parse(line);
            validatePetBucket(packet);
        }
        catch (error) {
            this.reset();
            throw error;
        }
        const previous = this.sequence;
        this.sequence = packet.sequence;
        if (previous === undefined || packet.sequence <= previous)
            return;
        return packet;
    }
}
exports.PetLiveTape = PetLiveTape;
const order = (a, b) => a < b ? -1 : a > b ? 1 : 0;
const keyOf = (e) => JSON.stringify([e.traceId, e.id]);
const isContainer = (e) => e.attributes['whalesong.container'] === true
    || e.attributes['codewhale.container'] === true;
/** Compile a single trace. Unknown-duration spans provide onsets, not occupancy.
 * Updates of the same trace/id replace earlier snapshots rather than double count.
 * An endpoint onset gets its own bucket; intervals use [start, end). */
function compilePetTelemetry(input, durationMs = 0, firstSequence = 0, originMs = 0) {
    if (input.length > 250_000)
        throw new Error('Pet input exceeds 250000 events.');
    if (!Number.isFinite(durationMs) || durationMs < 0)
        throw new Error('Invalid pet duration.');
    if (!Number.isSafeInteger(firstSequence) || firstSequence < 0)
        throw new Error('Invalid first pet bucket.');
    if (!Number.isFinite(originMs))
        throw new Error('Invalid pet clock origin.');
    durationMs = Math.max(0, durationMs - originMs);
    const unique = new Map();
    const traces = new Set();
    for (const e of input) {
        if (e.schemaVersion !== 1 || !e.id || !e.traceId || !model_js_1.CATEGORIES.includes(e.category)
            || !Number.isFinite(e.startTime) || !Number.isFinite(e.endTime)
            || e.startTime < 0 || e.endTime < e.startTime || !e.attributes)
            throw new Error('Invalid event-v1 pet input. Import through importTrace first.');
        traces.add(e.traceId);
        unique.set(keyOf(e), e);
    }
    if (traces.size > 1)
        throw new Error('Select one trace for the pet.');
    const parents = new Set([...unique.values()].filter(e => e.parentId).map(e => e.parentId));
    const events = [...unique.values()].filter(e => !isContainer(e)
        && !(e.category === 'orchestration' && parents.has(e.id)))
        .map(e => ({ ...e, startTime: e.startTime - originMs, endTime: (e.openEnded ? e.startTime : e.endTime) - originMs,
        attributes: e.attributes['whalesong.error_onset_ms'] === undefined ? e.attributes
            : { ...e.attributes, 'whalesong.error_onset_ms': (0, model_js_1.errorOnsetOf)(e) - originMs } }))
        .sort((a, b) => a.startTime - b.startTime || order(a.id, b.id));
    let lastOnset = 0;
    for (const e of events) {
        durationMs = Math.max(durationMs, e.endTime);
        lastOnset = Math.max(lastOnset, e.startTime);
    }
    const failures = events.filter(e => e.category === 'error' || e.status === 'error').map(model_js_1.errorOnsetOf).sort((a, b) => a - b);
    if (failures.length)
        lastOnset = Math.max(lastOnset, failures[failures.length - 1]);
    const count = Math.max(1, Math.ceil(durationMs / exports.PET_BIN_MS), Math.floor(lastOnset / exports.PET_BIN_MS) + 1);
    if (count - firstSequence > 216_000 || count > pet_sim_js_1.PET_MAX_SECONDS * 2.5)
        throw new Error('Pet replay exceeds 24 hours; select a shorter trace.');
    const index = new signal_js_1.IntervalIndex(events), result = [];
    const recent = [], names = new Map();
    let next = 0, expired = 0, nextFailure = 0;
    for (let sequence = firstSequence; sequence < count; sequence++) {
        const start = sequence * exports.PET_BIN_MS, end = start + exports.PET_BIN_MS;
        // A trailing window only: appending future events cannot rewrite earlier bins.
        while (next < events.length && events[next].startTime < end) {
            const e = events[next++];
            // A liveness pulse continues an operation; it is not a repeated tool call.
            if (e.attributes['whalesong.continuation'] === true)
                continue;
            recent.push(e);
            names.set(e.name, (names.get(e.name) ?? 0) + 1);
        }
        while (expired < recent.length && recent[expired].startTime < end - 12_000) {
            const name = recent[expired++].name, n = names.get(name) - 1;
            if (n)
                names.set(name, n);
            else
                names.delete(name);
        }
        const onsets = model_js_1.CATEGORIES.map(() => 0), activeMs = model_js_1.CATEGORIES.map(() => 0);
        const agents = new Set();
        while (nextFailure < failures.length && failures[nextFailure] < start)
            nextFailure++;
        let errors = 0, waiting = false, human = false;
        while (nextFailure < failures.length && failures[nextFailure] < end) {
            errors++;
            nextFailure++;
        }
        for (const e of index.query(start, end)) {
            if (e.startTime >= end)
                continue;
            const onset = e.startTime >= start, c = model_js_1.CATEGORIES.indexOf(e.category);
            const overlap = Math.max(0, Math.min(end, e.endTime) - Math.max(start, e.startTime));
            if (!onset && !overlap)
                continue;
            activeMs[c] += overlap;
            if (onset)
                onsets[c]++;
            if (e.agentId && !['unknown', 'unattributed'].includes(e.agentId))
                agents.add(e.agentId);
            if (e.category === 'human') {
                human = true;
                waiting ||= e.status === 'pending' || e.status === 'running' || e.attributes['whalesong.waiting'] === true;
            }
        }
        const total = activeMs.reduce((a, b) => a + b, 0), hits = onsets.reduce((a, b) => a + b, 0);
        const observed = total > 0 || hits > 0 || errors > 0;
        let dominant = model_js_1.CATEGORIES.indexOf('other');
        for (let c = 0; c < model_js_1.CATEGORIES.length; c++) {
            if (activeMs[c] > activeMs[dominant]
                || activeMs[c] === activeMs[dominant] && onsets[c] > onsets[dominant])
                dominant = c;
        }
        let repeated = 0;
        for (const n of names.values())
            if (n >= 4)
                repeated += n;
        const repeatDensity = repeated / Math.max(1, recent.length - expired);
        const channel = errors ? 'error' : human ? 'human' : agents.size >= 3 ? 'agent' : model_js_1.CATEGORIES[dominant];
        result.push({ version: 1, sequence, simTimeMs: start, durationMs: exports.PET_BIN_MS,
            activity: observed ? (0, model_js_1.clamp)(.28 + .38 * total / exports.PET_BIN_MS + .08 * hits, 0, 1) : .12,
            coherence: observed ? (0, model_js_1.clamp)(.92 - repeatDensity * .58 - Math.min(.55, errors * .18), .08, 1) : .25,
            attention: waiting ? .8 : human ? .65 : errors ? .45 : 0,
            channel, observed: observed ? 1 : 0, roamX: 0, roamY: 0, flip: 1, lit: 1,
            onsets, activeMs, errors, agentIds: [...agents].sort(order), waiting });
    }
    return result;
}
/** Flat state remains compatible with native readers; metadata preserves audio
 * onsets and peer identities. No prompts, tool arguments, or event names escape. */
function encodePetJSONL(buckets) {
    return buckets.map(b => JSON.stringify(b)).join('\n') + (buckets.length ? '\n' : '');
}
/** Legacy visual conformance interchange. Use JSONL to preserve audio metadata. */
function encodePetTSV(buckets) {
    return 'dt\tactivity\tcoherence\tattention\tchannel\tobserved\troamX\troamY\tflip\tlit\n'
        + buckets.map(b => [b.durationMs / 1000, b.activity, b.coherence, b.attention, b.channel,
            b.observed, b.roamX, b.roamY, b.flip, b.lit].join('\t')).join('\n') + '\n';
}

};
factories["signal"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.IntervalIndex = void 0;
exports.buildPyramid = buildPyramid;
exports.chooseLevel = chooseLevel;
exports.binValue = binValue;
exports.intensity = intensity;
exports.totals = totals;
exports.onsetSeries = onsetSeries;
exports.autocorrelation = autocorrelation;
exports.periodogram = periodogram;
exports.unionDuration = unionDuration;
const model_js_1 = require("./model.js");
function emptyLevel(length, binMs) {
    const n = model_js_1.CATEGORIES.length * length;
    return { binMs, length, onsets: new Float64Array(n), activeMs: new Float64Array(n),
        outputTokens: new Float64Array(n), cost: new Float64Array(n), errors: new Float64Array(n), peak: new Float64Array(n) };
}
const FIELDS = ['onsets', 'activeMs', 'outputTokens', 'cost', 'errors'];
/** O(events + channels × bins), including long intervals. No span-length inner loop. */
function buildPyramid(events, requestedDuration, maxBins = 16_384) {
    if (!Number.isInteger(maxBins) || maxBins < 16 || maxBins > 1_048_576)
        throw new Error('maxBins must be an integer in [16, 1048576].');
    let duration = requestedDuration ?? 1;
    for (const e of events)
        duration = Math.max(duration, e.endTime, e.startTime, e.status === 'error' ? (0, model_js_1.errorOnsetOf)(e) : 0);
    if (!Number.isFinite(duration) || duration < 0)
        throw new Error('Signal duration must be finite and nonnegative.');
    duration = Math.max(1, duration);
    const binMs = 2 ** Math.ceil(Math.log2(Math.max(1, duration / (maxBins - 1))));
    const length = Math.floor(duration / binMs) + 1, fine = emptyLevel(length, binMs);
    const stride = length + 1;
    const activeDiff = new Float64Array(model_js_1.CATEGORIES.length * stride), tokenDiff = new Float64Array(model_js_1.CATEGORIES.length * stride);
    for (const e of events) {
        if (!Number.isFinite(e.startTime) || !Number.isFinite(e.endTime) || e.startTime < 0 || e.endTime < e.startTime)
            throw new Error(`Invalid interval for ${e.id}.`);
        const channel = model_js_1.CATEGORIES.indexOf(e.category);
        if (channel < 0)
            throw new Error(`Unknown category for ${e.id}.`);
        const a = Math.floor(e.startTime / binMs), b = Math.floor(e.endTime / binMs);
        const at = channel * length, diff = channel * stride;
        fine.onsets[at + a]++;
        fine.cost[at + a] += e.cost ?? 0;
        if (e.status === 'error')
            fine.errors[at + Math.floor((0, model_js_1.errorOnsetOf)(e) / binMs)]++;
        const d = e.endTime - e.startTime, tokens = e.outputTokens ?? 0;
        if (d === 0) {
            fine.outputTokens[at + a] += tokens;
            continue;
        }
        if (a === b) {
            fine.activeMs[at + a] += d;
            fine.outputTokens[at + a] += tokens;
        }
        else {
            const left = (a + 1) * binMs - e.startTime, right = e.endTime - b * binMs, tokenRate = tokens / d;
            fine.activeMs[at + a] += left;
            fine.activeMs[at + b] += right;
            fine.outputTokens[at + a] += left * tokenRate;
            fine.outputTokens[at + b] += right * tokenRate;
            if (b > a + 1) {
                activeDiff[diff + a + 1] += binMs;
                activeDiff[diff + b] -= binMs;
                tokenDiff[diff + a + 1] += binMs * tokenRate;
                tokenDiff[diff + b] -= binMs * tokenRate;
            }
        }
    }
    for (let c = 0; c < model_js_1.CATEGORIES.length; c++) {
        let active = 0, tokens = 0;
        for (let i = 0; i < length; i++) {
            active += activeDiff[c * stride + i];
            tokens += tokenDiff[c * stride + i];
            const at = c * length + i;
            fine.activeMs[at] = Math.max(0, fine.activeMs[at] + active);
            fine.outputTokens[at] = Math.max(0, fine.outputTokens[at] + tokens);
            fine.peak[at] = fine.activeMs[at] / binMs;
        }
    }
    const levels = [fine];
    while (levels.at(-1).length > 1) {
        const child = levels.at(-1), parent = emptyLevel(Math.ceil(child.length / 2), child.binMs * 2);
        for (let c = 0; c < model_js_1.CATEGORIES.length; c++)
            for (let i = 0; i < parent.length; i++) {
                const a = c * child.length + i * 2, b = a + 1, dst = c * parent.length + i, hasB = i * 2 + 1 < child.length;
                for (const key of FIELDS)
                    parent[key][dst] = child[key][a] + (hasB ? child[key][b] : 0);
                parent.peak[dst] = Math.max(child.peak[a], hasB ? child.peak[b] : 0);
            }
        levels.push(parent);
    }
    const p = { duration, channels: model_js_1.CATEGORIES, levels, calibration: { activity: 1, onsets: 1, tokens: 1, cost: 1 } };
    const reference = chooseLevel(p, duration / 1200);
    for (const metric of ['activity', 'onsets', 'tokens', 'cost']) {
        const positives = [];
        for (let c = 0; c < model_js_1.CATEGORIES.length; c++)
            for (let i = 0; i < reference.length; i++) {
                const v = binValue(reference, c, i, metric);
                if (v > 0)
                    positives.push(v);
            }
        p.calibration[metric] = Math.max(1e-9, (0, model_js_1.quantile)(positives, .95));
    }
    return p;
}
/** Choose the coarsest stored level no wider than one requested pixel interval. */
function chooseLevel(p, targetBinMs) {
    let result = p.levels[0];
    for (const level of p.levels) {
        if (level.binMs > targetBinMs)
            break;
        result = level;
    }
    return result;
}
function binValue(level, channel, bin, metric) {
    if (bin < 0 || bin >= level.length)
        return 0;
    const at = channel * level.length + bin;
    if (metric === 'activity')
        return level.activeMs[at] / level.binMs;
    if (metric === 'onsets')
        return level.onsets[at] * 1000 / level.binMs;
    if (metric === 'tokens')
        return level.outputTokens[at] * 1000 / level.binMs;
    return level.cost[at] * 1000 / level.binMs;
}
function intensity(value, reference) {
    return (0, model_js_1.clamp)(Math.log1p(value / Math.max(1e-9, reference) * 8) / Math.log(9), 0, 1);
}
/** Conserved totals: useful for tests and alternate native backends. */
function totals(level) {
    return Object.fromEntries(FIELDS.map(k => [k, level[k].reduce((s, x) => s + x, 0)]));
}
/** Sorted-start, max-end segment tree. Long root spans do not force a reverse
 * scan through every earlier event. Results are chronological and honor limits. */
class IntervalIndex {
    events;
    maxEnd;
    leafCount;
    constructor(events) {
        this.events = [...events].sort((a, b) => a.startTime - b.startTime || a.id.localeCompare(b.id));
        this.leafCount = 2 ** Math.ceil(Math.log2(Math.max(1, this.events.length)));
        this.maxEnd = new Float64Array(this.leafCount * 2).fill(-Infinity);
        for (let i = 0; i < this.events.length; i++)
            this.maxEnd[this.leafCount + i] = this.events[i].endTime;
        for (let i = this.leafCount - 1; i; i--)
            this.maxEnd[i] = Math.max(this.maxEnd[i * 2], this.maxEnd[i * 2 + 1]);
    }
    query(start, end, limit = Infinity) {
        if (!Number.isFinite(start) || !Number.isFinite(end) || end < start || limit <= 0)
            return [];
        let lo = 0, hi = this.events.length;
        while (lo < hi) {
            const mid = (lo + hi) >>> 1;
            if (this.events[mid].startTime <= end)
                lo = mid + 1;
            else
                hi = mid;
        }
        const bound = lo, found = [];
        const visit = (node, left, right) => {
            if (left >= bound || this.maxEnd[node] < start || found.length >= limit)
                return;
            if (right - left === 1) {
                const e = this.events[left];
                if (e && (e.endTime > start || e.startTime === e.endTime && e.startTime >= start))
                    found.push(e);
                return;
            }
            const mid = (left + right) >>> 1;
            visit(node * 2, left, mid);
            visit(node * 2 + 1, mid, right);
        };
        visit(1, 0, this.leafCount);
        return found;
    }
}
exports.IntervalIndex = IntervalIndex;
/** Sample an onset train into equal-width bins; nothing is inferred between impulses. */
function onsetSeries(events, start, end, n = 256, category) {
    const out = new Float64Array(n), span = Math.max(1e-6, end - start);
    for (const e of events)
        if ((!category || e.category === category) && e.startTime >= start && e.startTime < end) {
            const at = Math.floor((e.startTime - start) / span * n);
            if (at >= 0 && at < n)
                out[at]++;
        }
    return out;
}
/** Centered, variance-normalized autocorrelation; lag-zero is one unless constant. */
function autocorrelation(input, maxLag = 64) {
    const n = input.length;
    if (!n)
        return [];
    let mean = 0;
    for (let i = 0; i < n; i++)
        mean += input[i];
    mean /= n;
    const centered = Array.from(input, x => x - mean), energy = centered.reduce((s, v) => s + v * v, 0);
    const result = [];
    for (let lag = 0; lag <= Math.min(maxLag, n - 1); lag++) {
        let sum = 0;
        for (let i = 0; i < n - lag; i++)
            sum += centered[i] * centered[i + lag];
        result.push(energy > 1e-12 ? sum / energy : 0);
    }
    return result;
}
/** Hann-window periodogram of an actual uniformly sampled onset signal, DC removed. */
function periodogram(input, sampleHz) {
    const n = input.length, frequencies = [], power = [];
    if (n < 4 || sampleHz <= 0)
        return { frequencies, power, entropy: 0, peakHz: 0, sampleHz, resolutionHz: 0 };
    let mean = 0;
    for (let i = 0; i < n; i++)
        mean += input[i];
    mean /= n;
    const windowed = Array.from(input, (x, i) => (x - mean) * (.5 - .5 * Math.cos(2 * Math.PI * i / (n - 1))));
    const windowEnergy = Array.from({ length: n }, (_, i) => (.5 - .5 * Math.cos(2 * Math.PI * i / (n - 1))) ** 2).reduce((a, b) => a + b, 0);
    for (let k = 1; k <= Math.floor(n / 2); k++) {
        let re = 0, im = 0;
        for (let t = 0; t < n; t++) {
            const angle = 2 * Math.PI * k * t / n;
            re += windowed[t] * Math.cos(angle);
            im -= windowed[t] * Math.sin(angle);
        }
        frequencies.push(k * sampleHz / n);
        power.push((re * re + im * im) / (windowEnergy * sampleHz) * (n % 2 === 0 && k === n / 2 ? 1 : 2));
    }
    const sum = power.reduce((s, x) => s + x, 0);
    let entropy = 0;
    for (const x of power)
        if (x > 0 && sum > 0) {
            const p = x / sum;
            entropy -= p * Math.log2(p);
        }
    entropy = power.length > 1 ? entropy / Math.log2(power.length) : 0;
    const max = Math.max(...power);
    return { frequencies, power, entropy, peakHz: max > 1e-12 ? frequencies[power.indexOf(max)] : 0, sampleHz, resolutionHz: sampleHz / n };
}
function unionDuration(events, start = 0, end = Infinity) {
    const intervals = events.filter(e => e.endTime > e.startTime && e.endTime > start && e.startTime < end)
        .map(e => [Math.max(start, e.startTime), Math.min(end, e.endTime)]).sort((a, b) => a[0] - b[0]);
    let total = 0, left = 0, right = 0;
    for (const [a, b] of intervals) {
        if (a > right) {
            total += right - left;
            left = a;
            right = b;
        }
        else
            right = Math.max(right, b);
    }
    return total + right - left;
}

};
factories["pet-audio"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetScore = void 0;
exports.renderPetPCM = renderPetPCM;
const pet_sim_js_1 = require("./pet-sim.js");
const model_js_1 = require("./model.js");
/** Core emits score events; WebAudio / AVAudioEngine only present their PCM.
 * Calling at any display cadence produces the same score when all world ticks
 * are supplied. Calling twice for a world tick cannot retrigger a voice. */
class PetScore {
    lastWindow = -1;
    lastSequence = -1;
    lastAddress = false;
    checkpoint() { return [this.lastWindow, this.lastSequence, this.lastAddress]; }
    restore(value) {
        if (!Array.isArray(value) || value.length !== 3
            || !value.slice(0, 2).every(n => Number.isSafeInteger(n) && n >= -1 && n <= pet_sim_js_1.PET_MAX_SECONDS * 2.5) || typeof value[2] !== 'boolean')
            throw new Error('Invalid pet score checkpoint.');
        [this.lastWindow, this.lastSequence, this.lastAddress] = value;
    }
    voices(frame) {
        const time = frame.timeMs / 1000, window = Math.floor((frame.timeMs + 1e-7) / 400);
        const out = [];
        const add = (id, frequency, duration, gain, pan = 0, delay = 0, kind = 'tone') => out.push({ id, start: time + delay, duration, frequency, gain, pan, kind });
        const t = frame.telemetry, fresh = t !== undefined && t.sequence !== this.lastSequence;
        if (fresh) {
            this.lastSequence = t.sequence;
            for (let c = 0; c < pet_sim_js_1.CHANNELS.length; c++) {
                const channel = pet_sim_js_1.CHANNELS[c], n = t.onsets[c];
                if (!n || channel.sustained || ['human', 'error'].includes(channel.key))
                    continue;
                add(`onset:${t.sequence}:${c}`, channel.freq, .24, .035 * Math.min(2, Math.sqrt(n)), (c / 12 - .5) * .7);
            }
            if (t.errors)
                add(`tear:${t.sequence}`, pet_sim_js_1.CHANNELS.find(c => c.key === 'error').freq, .22, .05, 0, 0, 'noise');
        }
        const address = frame.state.channel === 'human' && frame.state.attention > .5;
        if (address && (!this.lastAddress || fresh && t.onsets[11] > 0)) {
            const frequency = pet_sim_js_1.CHANNELS[11].freq;
            for (let i = 0; i < 3; i++)
                add(`address:${frame.timeMs}:${i}`, frequency * [1, 1.25, 1.5][i], .23, .032, 0, i * .14);
        }
        this.lastAddress = address;
        if (window !== this.lastWindow) {
            this.lastWindow = window;
            if (frame.state.observed >= .92) {
                const channel = pet_sim_js_1.CHANNELS.find(c => c.key === frame.state.channel);
                if (channel.sustained && frame.behaviour !== 'doze') {
                    const peers = frame.state.channel === 'agent' ? Math.max(1, frame.pod.filter(p => p.present).length) : 1;
                    for (let i = 0; i < peers; i++)
                        add(`sustain:${window}:${i}`, channel.freq * (1 + (i - (peers - 1) / 2) * .004), .44, (.02 + frame.state.activity * .02) / Math.sqrt(peers), peers === 1 ? 0 : i / (peers - 1) - .5);
                }
                if (frame.behaviour === 'doze' && window % 5 === 0) {
                    add(`heart:${window}:0`, 49, .3, .035);
                    add(`heart:${window}:1`, 49, .24, .023, 0, .33);
                }
                if (frame.needs === 'call' && window % 10 === 0)
                    add(`call:${window}`, pet_sim_js_1.CHANNELS[11].freq * 1.5, .38, .03);
            }
        }
        return out;
    }
}
exports.PetScore = PetScore;
/** Sample-addressed noise: no global RNG, identical samples when chunked/seeking. */
function noise(seed, sample) {
    let x = (seed + Math.imul(sample, 0x6D2B79F5)) >>> 0;
    x = Math.imul(x ^ x >>> 15, x | 1);
    x ^= x + Math.imul(x ^ x >>> 7, x | 61);
    return ((x ^ x >>> 14) >>> 0) / 2147483648 - 1;
}
function renderPetPCM(voices, startSample, length, sampleRate = 48_000) {
    if (!Number.isInteger(sampleRate) || sampleRate < 8000 || sampleRate > 96000
        || !Number.isSafeInteger(startSample) || startSample < 0 || !Number.isInteger(length) || length < 0 || length > sampleRate * 120)
        throw new Error('Invalid pet PCM range.');
    const left = new Float32Array(length), right = new Float32Array(length);
    for (const v of voices) {
        if (![v.start, v.duration, v.frequency, v.gain, v.pan].every(Number.isFinite)
            || v.start < 0 || v.duration <= 0 || v.duration > 10 || v.frequency <= 0 || v.frequency > sampleRate / 2
            || v.gain < 0 || v.gain > 1 || Math.abs(v.pan) > 1 || !['tone', 'noise'].includes(v.kind))
            throw new Error('Invalid pet voice.');
        const first = Math.max(startSample, Math.ceil(v.start * sampleRate));
        const last = Math.min(startSample + length, Math.ceil((v.start + v.duration) * sampleRate));
        const pan = (v.pan + 1) * Math.PI / 4, seed = (0xC0FFEE ^ (0, model_js_1.stableHash)(v.id)) >>> 0;
        for (let absolute = first; absolute < last; absolute++) {
            const age = absolute / sampleRate - v.start;
            const envelope = Math.min(1, age / .015, (v.duration - age) / .045);
            const sample = v.kind === 'noise' ? noise(seed, absolute) * Math.exp(-age * 14)
                : Math.sin(2 * Math.PI * v.frequency * age) * .88 + Math.sin(4 * Math.PI * v.frequency * age) * .12;
            const value = sample * Math.max(0, envelope) * v.gain, at = absolute - startSample;
            left[at] += value * Math.cos(pan);
            right[at] += value * Math.sin(pan);
        }
    }
    // Limiting is a presentation operation and cannot perturb voice scheduling.
    for (let i = 0; i < length; i++) {
        left[i] = (0, model_js_1.clamp)(left[i], -1, 1);
        right[i] = (0, model_js_1.clamp)(right[i], -1, 1);
    }
    return { left, right };
}

};
factories["pet-engine"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PetEngineTelemetry = void 0;
const pet_sim_js_1 = require("./pet-sim.js");
const codewhale_js_1 = require("./codewhale.js");
const pet_telemetry_js_1 = require("./pet-telemetry.js");
/** Read-only adapter for codewhale_protocol::EventMsg metadata. The foreground
 * Engine is the event owner. This replaces no turn loop: it only translates
 * lifecycle observations to event-v1 for the same pet bucketer used by imports.
 * Text, inputs and results are neither accepted nor retained. */
class PetEngineTelemetry {
    events = [];
    active = new Map();
    waiting;
    sequence = 0;
    lastTime = 0;
    add(name, category, at, agentId = 'parent', continuation = false) {
        if (this.events.length >= 8192)
            throw new Error('Pet Engine observation window is full.');
        const e = { schemaVersion: 1, id: `engine:${this.sequence++}`, traceId: 'foreground',
            startTime: at, endTime: at, name, category, agentId, status: 'running',
            attributes: continuation ? { 'whalesong.continuation': true } : {} };
        this.events.push(e);
        return e;
    }
    pulse(key, at) {
        const e = this.active.get(key);
        if (!e)
            return;
        // A resumed stream does not assert coverage across its silent interval.
        if (at - e.endTime > pet_telemetry_js_1.PET_BIN_MS * 2) {
            this.active.set(key, this.add(e.name, e.category, at, e.agentId, true));
        }
        else
            e.endTime = at;
    }
    observe(value, at) {
        if (!Number.isFinite(at) || at < this.lastTime || at > pet_sim_js_1.PET_MAX_SECONDS * 1000)
            throw new Error('Invalid Engine pet clock.');
        if (!value || typeof value !== 'object' || Array.isArray(value))
            throw new Error('Invalid Engine pet metadata.');
        const e = value;
        const allowed = ['event', 'index', 'channel', 'tool_call_id', 'tool_name', 'id', 'worker_status', 'failed'];
        if (Object.keys(e).some(k => !allowed.includes(k)) || typeof e.event !== 'string'
            || Object.values(e).some(v => typeof v === 'string' && v.length > 4096)
            || e.channel !== undefined && !['text', 'reasoning'].includes(e.channel)
            || ['tool_call_id', 'tool_name', 'id', 'worker_status'].some(k => e[k] !== undefined && typeof e[k] !== 'string')
            || e.failed !== undefined && typeof e.failed !== 'boolean'
            || e.index !== undefined && (!Number.isSafeInteger(e.index) || e.index < 0))
            throw new Error('Invalid Engine pet metadata fields.');
        this.lastTime = at;
        this.events = this.events.filter(span => span.endTime >= at - 12_800);
        const id = (field) => { const s = e[field]; if (typeof s !== 'string' || !s)
            throw new Error(`Missing Engine ${field}.`); return s; };
        const index = () => { if (!Number.isSafeInteger(e.index))
            throw new Error('Missing Engine index.'); return String(e.index); };
        const start = (key, name, category, agentId) => {
            if (this.active.size >= 256 && !this.active.has(key))
                throw new Error('Too many active Engine pet spans.');
            this.active.set(key, this.add(name, category, at, agentId));
        };
        const finish = (key) => { this.pulse(key, at); this.active.delete(key); };
        switch (e.event) {
            case 'turn_started':
                this.active.clear();
                this.waiting = undefined;
                break;
            case 'message_started':
                start(`message:${index()}`, 'assistant_message', 'communication');
                this.waiting = undefined;
                break;
            case 'thinking_started':
                start(`thinking:${index()}`, 'thinking', 'reasoning');
                this.waiting = undefined;
                break;
            case 'response_delta': {
                const reasoning = e.channel === 'reasoning';
                const key = `${reasoning ? 'thinking' : 'message'}:${index()}`;
                if (!this.active.has(key))
                    start(key, reasoning ? 'thinking' : 'assistant_message', reasoning ? 'reasoning' : 'communication');
                else
                    this.pulse(key, at);
                this.waiting = undefined;
                break;
            }
            case 'message_complete':
                finish(`message:${index()}`);
                break;
            case 'thinking_complete':
                finish(`thinking:${index()}`);
                break;
            case 'tool_call_started':
                start(`tool:${id('tool_call_id')}`, id('tool_name'), (0, codewhale_js_1.toolCategory)(id('tool_name')));
                this.waiting = undefined;
                break;
            case 'tool_call_heartbeat':
                for (const key of this.active.keys())
                    if (key.startsWith('tool:'))
                        this.pulse(key, at);
                break;
            case 'tool_call_complete':
                finish(`tool:${id('tool_call_id')}`);
                this.waiting = undefined;
                break;
            case 'agent_spawned':
                start(`agent:${id('id')}`, 'agent', 'agent', id('id'));
                break;
            case 'agent_progress': {
                const key = `agent:${id('id')}`;
                if (['completed', 'failed', 'cancelled', 'interrupted', 'budget_exhausted'].includes(e.worker_status)) {
                    finish(key);
                    break;
                }
                if (!this.active.has(key))
                    start(key, 'agent', 'agent', id('id'));
                else
                    this.pulse(key, at);
                break;
            }
            case 'agent_complete':
                finish(`agent:${id('id')}`);
                break;
            case 'approval_required':
            case 'user_input_required':
                this.waiting = this.add('human', 'human', at);
                this.waiting.status = 'pending';
                break;
            case 'turn_complete':
                this.active.clear();
                this.waiting = undefined;
                break;
            case 'error': break;
            default: throw new Error('Unsupported Engine pet event.');
        }
        // Receipt time is the error onset; never rewrite the operation's old start.
        if (e.event === 'error' || e.failed === true)
            this.add('error', 'error', at).status = 'error';
    }
    /** Waiting coverage comes from the existing typed shell's current request.
     * It can extend a witnessed request, never invent one on a mid-turn attach. */
    confirmWaiting(at, waiting) {
        if (!waiting) {
            this.waiting = undefined;
            return;
        }
        if (this.waiting && at >= this.waiting.endTime)
            this.waiting.endTime = at;
    }
    bucket(sequence) {
        const end = (sequence + 1) * pet_telemetry_js_1.PET_BIN_MS;
        const input = this.events.filter(e => e.startTime < end && e.endTime >= end - 12_400)
            .map(e => ({ ...e, endTime: Math.min(e.endTime, end) }));
        return (0, pet_telemetry_js_1.compilePetTelemetry)(input, end, sequence)[0];
    }
}
exports.PetEngineTelemetry = PetEngineTelemetry;

};
factories["codewhale"]=function(exports,require){
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.isCodewhaleSession = isCodewhaleSession;
exports.isCodewhaleRuntimeRecord = isCodewhaleRuntimeRecord;
exports.isCodewhaleRuntimeDocument = isCodewhaleRuntimeDocument;
exports.toolCategory = toolCategory;
exports.fromCodewhaleSession = fromCodewhaleSession;
exports.fromCodewhaleRuntime = fromCodewhaleRuntime;
exports.observeRuntimeRequests = observeRuntimeRequests;
/** Read-only Codewhale session/runtime adapter. Does not record, mutate, or own receipts. */
const model_js_1 = require("./model.js");
const PAYLOAD_LIMIT = 2000;
const COLLAPSED_SPAN_MS = 1000;
const ENVELOPE_MIN_MS = 60_000;
const obj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v) ? v : {};
const str = (v) => typeof v === 'string' && v.length ? v : undefined;
const num = (v) => typeof v === 'number' && Number.isFinite(v) ? v : undefined;
function isCodewhaleSession(value) {
    const root = obj(value);
    const metadata = obj(root.metadata);
    if (!str(metadata.id))
        return false;
    if (root.format === 'whalesong.evidence/v1' || Array.isArray(root.resourceSpans) || root.schemaVersion === 1)
        return false;
    const journal = obj(root.journal);
    return Array.isArray(root.messages) || Array.isArray(journal.entries);
}
function isCodewhaleRuntimeRecord(value) {
    const rec = obj(value);
    return Number.isSafeInteger(rec.seq) && rec.seq >= 0 && typeof rec.event === 'string' && !!rec.event
        && typeof rec.thread_id === 'string' && !!rec.thread_id && rec.timestamp != null;
}
function isCodewhaleRuntimeDocument(value) {
    if (!Array.isArray(value) || !value.length)
        return false;
    const n = Math.min(value.length, 8);
    let hits = 0;
    for (let i = 0; i < n; i++)
        if (isCodewhaleRuntimeRecord(value[i]))
            hits++;
    return hits === n;
}
function clip(value) {
    if (value == null)
        return value;
    const text = typeof value === 'string' ? value : JSON.stringify(value);
    if (text.length <= PAYLOAD_LIMIT)
        return typeof value === 'string' ? value : JSON.parse(text);
    return `${text.slice(0, PAYLOAD_LIMIT)}…[truncated ${text.length - PAYLOAD_LIMIT} source bytes]`;
}
function parseTime(value) {
    if (typeof value === 'number' && Number.isFinite(value))
        return value;
    if (typeof value !== 'string' || !value)
        return undefined;
    const ms = Date.parse(value);
    return Number.isFinite(ms) ? ms : undefined;
}
function statusOf(value, isError) {
    if (isError === true)
        return 'error';
    if (isError === false)
        return 'success';
    const s = String(value ?? '').toLowerCase();
    if (s === 'completed' || s === 'success' || s === 'ok')
        return 'success';
    if (s === 'failed' || s === 'error' || s === 'errored')
        return 'error';
    if (s === 'canceled' || s === 'cancelled' || s === 'interrupted')
        return 'error';
    if (s === 'in_progress' || s === 'running')
        return 'running';
    if (s === 'pending')
        return 'pending';
    return 'unknown';
}
function classify(name) {
    const n = name.toLowerCase();
    if (/exception|^error\b/.test(n))
        return 'error';
    if (/spawn|fork|subagent|^agent$/.test(n))
        return 'agent';
    if (/message\.send|handoff|agent\.message|assistant_message/.test(n))
        return 'communication';
    if (/retrieve|retrieval|context|embedding|vector|memory|rag/.test(n))
        return 'memory';
    if (/browser|navigate|screenshot|click|playwright/.test(n))
        return 'browser';
    if (/read_file|write_file|list_dir|^read$|^write$|^edit$|glob|grep|file\.|filesystem/.test(n))
        return 'filesystem';
    if (/bash|exec|shell|run_test|cargo|pytest|compile/.test(n))
        return 'code';
    if (/reason|thinking|completion|generate|chat|llm/.test(n))
        return 'reasoning';
    if (/http|request|api|fetch|network|mcp_/.test(n))
        return 'network';
    if (/user_message|human|approval/.test(n))
        return 'human';
    if (/orchestrat|workflow|phase|join|session|thread|turn|todo|plan|operate_contract|status/.test(n))
        return 'orchestration';
    if (/tool/.test(n))
        return 'tool';
    return model_js_1.CATEGORIES.includes(n) ? n : 'other';
}
function toolCategory(name) {
    const category = classify(name);
    return category === 'other' ? 'tool' : category;
}
function pointer(source, ids) {
    return { format: source, ...ids };
}
function titleOfSession(metadata, filename) {
    const title = str(metadata.title)?.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
    if (title && !title.startsWith('codewhale:runtime_event'))
        return title.slice(0, 120);
    return `Codewhale session · ${(str(metadata.id) ?? filename).slice(0, 8)}`;
}
function activeJournalEntries(journal) {
    const entries = Array.isArray(journal.entries) ? journal.entries.map(obj) : [];
    const leaf = str(journal.leaf_id);
    if (!leaf || !entries.length)
        return { entries, warnings: [] };
    const byId = new Map(entries.filter(e => str(e.id)).map(e => [e.id, e]));
    const chain = [];
    const seen = new Set();
    let id = leaf;
    while (id && !seen.has(id)) {
        seen.add(id);
        const entry = byId.get(id);
        if (!entry)
            break;
        chain.push(entry);
        id = str(entry.parent_id);
    }
    if (!chain.length)
        return { entries, warnings: ['Journal leaf_id did not resolve; using append order instead of the active branch.'] };
    if (chain.length < entries.length) {
        return {
            entries: chain.reverse(),
            warnings: [`Active journal branch has ${chain.length} of ${entries.length} entries. Forked history was not invented into the timeline.`],
        };
    }
    return { entries: chain.reverse(), warnings: [] };
}
function collapsedTimestamps(entries, created, updated) {
    const times = entries.map(e => parseTime(e.created_at)).filter((n) => n !== undefined);
    if (times.length < 2)
        return false;
    const span = Math.max(...times) - Math.min(...times);
    const envelope = created !== undefined && updated !== undefined ? updated - created : 0;
    return envelope >= ENVELOPE_MIN_MS && span < COLLAPSED_SPAN_MS;
}
function pushEvent(events, event) {
    events.push(event);
}
function fromCodewhaleSession(document, filename = 'Codewhale session', maxEvents = 250_000) {
    const root = obj(document);
    const metadata = obj(root.metadata);
    const sessionId = str(metadata.id) ?? filename;
    const journal = obj(root.journal);
    const { entries, warnings } = activeJournalEntries(journal);
    const sourceEntries = entries.length ? entries : (Array.isArray(root.messages) ? root.messages.map((message, i) => ({ id: `${sessionId}/message/${i}`, kind: 'message', message })) : []);
    if (!sourceEntries.length)
        throw new Error('Codewhale session contains no journal entries or messages.');
    const created = parseTime(metadata.created_at);
    const updated = parseTime(metadata.updated_at);
    const orderOnly = collapsedTimestamps(sourceEntries, created, updated);
    if (orderOnly) {
        warnings.push('Journal created_at values are collapsed to last-save time, not execution time. The time axis is journal order (1 ms per emitted event), not wall-clock duration. Gap, burst, and cycle-period findings are not execution-time claims.');
    }
    else {
        const times = sourceEntries.map(e => parseTime(e.created_at)).filter((n) => n !== undefined);
        if (!times.length)
            warnings.push('Journal entries have no usable timestamps. The time axis is journal order.');
    }
    const events = [];
    const pending = new Map();
    let seq = 0;
    const originWall = orderOnly ? undefined : sourceEntries.map(e => parseTime(e.created_at)).find((n) => n !== undefined);
    const agentId = 'parent';
    const model = str(metadata.model);
    const provider = str(metadata.model_provider);
    const when = (entry, fallback) => {
        if (orderOnly || originWall === undefined)
            return { start: fallback, open: false };
        const t = parseTime(entry.created_at);
        if (t === undefined)
            return { start: fallback, open: true };
        return { start: t - originWall, open: false };
    };
    for (const entry of sourceEntries) {
        if (events.length >= maxEvents)
            throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
        const entryId = str(entry.id) ?? `${sessionId}/entry/${seq}`;
        const message = obj(entry.message ?? (entry.kind === 'message' ? entry : {}));
        const role = str(message.role) ?? (str(entry.kind) === 'user' ? 'user' : str(entry.kind) === 'assistant' ? 'assistant' : undefined);
        const blocks = Array.isArray(message.content) ? message.content.map(obj) : [];
        if (!blocks.length) {
            const text = str(entry.text) ?? str(message.text);
            if (text)
                blocks.push({ type: role === 'user' ? 'text' : 'text', text });
        }
        if (!blocks.length)
            continue;
        const parentEventId = events.length ? events[events.length - 1].id : undefined;
        for (const block of blocks) {
            const t = when(entry, seq);
            const idBase = `${entryId}/${seq}`;
            const type = str(block.type) ?? 'text';
            const raw = pointer('codewhale.session/v1', { sessionId, entryId, seq, blockType: type, toolUseId: block.id ?? block.tool_use_id });
            if (type === 'tool_use' || type === 'server_tool_use') {
                const tool = str(block.name) ?? 'tool';
                const callId = str(block.id) ?? idBase;
                const started = tool === 'agent' && obj(block.input).action === 'start';
                const event = {
                    schemaVersion: 1, id: callId, traceId: sessionId, parentId: parentEventId,
                    startTime: t.start, endTime: t.start, openEnded: true,
                    agentId, name: started ? 'agent.spawn' : tool, tool, category: toolCategory(tool),
                    subtype: started ? 'fork' : undefined, model, provider,
                    status: 'running', attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, 'tool.name': tool },
                    payload: { arguments: clip(block.input) }, raw,
                };
                pending.set(callId, events.length);
                pushEvent(events, event);
            }
            else if (type === 'tool_result') {
                const callId = str(block.tool_use_id);
                const isError = block.is_error === true;
                const target = callId !== undefined ? pending.get(callId) : undefined;
                if (target !== undefined) {
                    const prior = events[target];
                    prior.endTime = t.start;
                    prior.openEnded = false;
                    prior.status = statusOf('completed', isError);
                    prior.payload = { ...(obj(prior.payload)), result: clip(block.content) };
                    prior.attributes = { ...prior.attributes, 'codewhale.result_entry_id': entryId };
                    pending.delete(callId);
                }
                else {
                    pushEvent(events, {
                        schemaVersion: 1, id: idBase, traceId: sessionId, parentId: callId ?? parentEventId,
                        startTime: t.start, endTime: t.start, agentId,
                        name: 'tool_result', category: 'tool', model, provider,
                        status: statusOf(undefined, isError),
                        attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, tool_use_id: callId },
                        payload: { result: clip(block.content) }, raw,
                    });
                }
            }
            else if (type === 'thinking') {
                pushEvent(events, {
                    schemaVersion: 1, id: idBase, traceId: sessionId, parentId: parentEventId,
                    startTime: t.start, endTime: t.start, agentId, name: 'thinking', category: 'reasoning',
                    model, provider, status: 'success',
                    attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq },
                    payload: { thinking: clip(block.thinking ?? block.text) }, raw,
                });
            }
            else {
                const text = str(block.text) ?? '';
                const operate = text.includes('codewhale:runtime_event');
                const user = role === 'user' || role === 'User';
                pushEvent(events, {
                    schemaVersion: 1, id: idBase, traceId: sessionId, parentId: parentEventId,
                    startTime: t.start, endTime: t.start, agentId,
                    name: operate ? 'operate_contract' : user ? 'user_message' : 'assistant_message',
                    category: operate ? 'orchestration' : user ? 'human' : 'communication',
                    model, provider, status: 'success',
                    attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, role: role ?? 'unknown' },
                    payload: { text: clip(text) }, raw,
                });
            }
            seq += 1;
        }
    }
    if (!events.length)
        throw new Error('Codewhale session produced no inspectable events.');
    for (const event of events) {
        if (event.openEnded && event.tool)
            warnings.push(`Tool ${event.id} has no matching tool_result in this snapshot; duration remains unknown.`);
    }
    const base = events.reduce((m, e) => Math.min(m, e.startTime), events[0].startTime);
    for (const event of events) {
        event.startTime -= base;
        event.endTime -= base;
    }
    const cost = obj(metadata.cost);
    const sessionCost = num(cost.session_cost_usd);
    const duration = Math.max(1, events.reduce((m, e) => Math.max(m, e.endTime, e.startTime), 0));
    const uniqueWarnings = [...new Set(warnings)];
    return {
        id: sessionId,
        name: titleOfSession(metadata, filename),
        events,
        duration,
        originTime: orderOnly ? 'journal-order' : (str(metadata.created_at) ?? `${base} ms`),
        source: 'codewhale',
        privacy: 'redact',
        warnings: uniqueWarnings,
        metadata: {
            sourceFormat: 'codewhale.session/v1',
            timeBasis: orderOnly || originWall === undefined ? 'journal-order' : 'wall-clock',
            sourceFilename: filename,
            sessionId,
            model,
            provider,
            workspace: metadata.workspace,
            mode: metadata.mode,
            envelopeCreatedAt: metadata.created_at,
            envelopeUpdatedAt: metadata.updated_at,
            cumulativeTurnSecs: metadata.cumulative_turn_secs,
            messageCount: metadata.message_count,
            journalEntries: sourceEntries.length,
            totalTokens: metadata.total_tokens,
            sessionCostUsd: sessionCost,
            pricedTurns: cost.priced_turns,
            unpricedTurns: cost.unpriced_turns,
            runtimeStore: metadata.runtime_store,
            timeUnit: 'ms',
        },
    };
}
function itemToolName(item, payload) {
    const named = str(payload.tool) ?? str(item.tool) ?? str(item.name);
    if (named)
        return named;
    if (str(item.kind) !== 'tool_call')
        return undefined;
    const head = str(item.summary)?.split(':')[0]?.trim();
    if (head && head.length < 80 && !/\s/.test(head))
        return head;
    return undefined;
}
function itemCategory(kind, tool) {
    if (kind === 'user_message')
        return 'human';
    if (kind === 'agent_reasoning')
        return 'reasoning';
    if (kind === 'agent_message')
        return 'communication';
    if (kind === 'status')
        return 'orchestration';
    if (kind === 'tool_call' && tool)
        return toolCategory(tool);
    if (kind === 'tool_call')
        return 'tool';
    return classify(kind);
}
function fromCodewhaleRuntime(records, filename = 'Codewhale runtime', maxEvents = 250_000) {
    if (!records.length)
        throw new Error('Codewhale runtime event file is empty.');
    const first = obj(records[0]);
    const threadId = str(first.thread_id) ?? filename;
    const warnings = [];
    let skippedDeltas = 0;
    const open = new Map();
    const requests = new Map();
    const events = [];
    let origin;
    let model;
    let threadName = threadId;
    const stamp = (rec) => {
        const t = parseTime(rec.timestamp);
        if (t === undefined)
            throw new Error(`Runtime event seq ${rec.seq} is missing a usable timestamp.`);
        if (origin === undefined)
            origin = t;
        return t - origin;
    };
    for (const raw of records) {
        if (!isCodewhaleRuntimeRecord(raw))
            throw new Error('Runtime import cancelled: a line is not a Codewhale runtime event record. No rows were skipped.');
        const rec = obj(raw);
        if (rec.thread_id !== threadId)
            throw new Error('Runtime import contains multiple threads. Export one thread before importing.');
        const eventName = rec.event;
        if (eventName === 'item.delta') {
            skippedDeltas++;
            continue;
        }
        if (events.length >= maxEvents)
            throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
        const payload = obj(rec.payload);
        const item = obj(payload.item);
        const turn = obj(payload.turn);
        const thread = obj(payload.thread);
        const relative = stamp(rec);
        const turnId = str(rec.turn_id) ?? str(payload.turn_id);
        const itemId = str(rec.item_id) ?? str(item.id);
        const agentId = 'parent';
        if (str(thread.model))
            model = str(thread.model);
        if (str(turn.model))
            model = str(turn.model) ?? model;
        if (eventName === 'thread.started') {
            model = str(thread.model) ?? model;
            threadName = str(thread.id) ?? threadId;
            pushEvent(events, {
                schemaVersion: 1, id: `thread:${threadId}`, traceId: threadId,
                startTime: relative, endTime: relative, openEnded: true,
                agentId, name: 'thread', category: 'orchestration', model, status: 'running',
                attributes: { 'codewhale.seq': rec.seq, 'whalesong.container': true }, raw: rec,
            });
            continue;
        }
        if (eventName === 'turn.started' || eventName === 'turn.completed') {
            const id = `turn:${turnId ?? rec.seq}`;
            if (eventName === 'turn.completed')
                for (const [key, request] of requests) {
                    if (request.parentId !== id)
                        continue;
                    request.endTime = Math.max(request.startTime, relative);
                    request.openEnded = false;
                    request.status = 'unknown';
                    requests.delete(key);
                }
            const startWall = parseTime(turn.started_at) ?? parseTime(turn.created_at);
            const endWall = parseTime(turn.ended_at);
            const start = startWall !== undefined && origin !== undefined ? startWall - origin : relative;
            const end = eventName === 'turn.completed' && endWall !== undefined && origin !== undefined ? endWall - origin : relative;
            const usage = obj(turn.usage);
            const existing = events.findIndex(e => e.id === id);
            const next = {
                schemaVersion: 1, id, traceId: threadId, parentId: `thread:${threadId}`,
                startTime: start, endTime: Math.max(start, end), openEnded: eventName !== 'turn.completed',
                agentId, name: 'turn', category: 'orchestration', model,
                inputTokens: num(usage.input_tokens), outputTokens: num(usage.output_tokens),
                status: statusOf(turn.status ?? payload.status), latency: num(turn.duration_ms),
                attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'whalesong.container': true,
                    ...(statusOf(turn.status ?? payload.status) === 'error' ? { 'whalesong.error_onset_ms': relative } : {}) },
                payload: { input_summary: clip(turn.input_summary) }, raw: rec,
            };
            if (existing >= 0)
                events[existing] = { ...events[existing], ...next, startTime: events[existing].startTime };
            else
                pushEvent(events, next);
            continue;
        }
        if (eventName === 'turn.lifecycle')
            continue;
        if (['approval.required', 'approval.decided', 'approval.timeout', 'user_input.required', 'user_input.answered', 'user_input.canceled'].includes(eventName)) {
            const kind = eventName.startsWith('approval.') ? 'approval' : 'user_input';
            const requestId = str(payload[kind === 'approval' ? 'approval_id' : 'input_id']) ?? str(payload.id);
            if (!requestId)
                throw new Error(`Runtime ${eventName} is missing its request identity.`);
            const key = JSON.stringify([turnId ?? '', kind, requestId]);
            const prior = requests.get(key), required = eventName.endsWith('.required');
            if (required && prior)
                continue;
            if (!required && prior) {
                prior.endTime = Math.max(prior.startTime, relative);
                prior.openEnded = false;
                prior.status = eventName === 'approval.decided' || eventName === 'user_input.answered' ? 'success' : 'unknown';
                if (payload.auto === true) {
                    // Automatic consent has a receipt, but never asked the human to wait.
                    prior.category = 'orchestration';
                    delete prior.attributes['whalesong.waiting'];
                    prior.attributes['whalesong.container'] = true;
                }
                requests.delete(key);
                continue;
            }
            const automatic = payload.auto === true;
            const event = {
                schemaVersion: 1, id: `request:${key}:${rec.seq}`, traceId: threadId,
                parentId: turnId ? `turn:${turnId}` : undefined, startTime: relative, endTime: relative,
                openEnded: required, agentId, name: eventName, category: automatic ? 'orchestration' : 'human',
                status: required ? 'pending' : 'success', model,
                attributes: { 'codewhale.seq': rec.seq, 'whalesong.waiting': required, 'whalesong.container': automatic }, raw: rec,
            };
            if (required)
                requests.set(key, event);
            pushEvent(events, event);
            continue;
        }
        if (eventName === 'tool_call.requested' || eventName === 'tool_call.canceled') {
            const callId = str(payload.call_id) ?? `call:${rec.seq}`;
            const tool = str(payload.tool);
            const canceled = eventName === 'tool_call.canceled';
            pushEvent(events, {
                schemaVersion: 1, id: `${eventName}:${callId}`, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
                startTime: relative, endTime: relative, agentId, name: tool ?? eventName, tool,
                category: tool ? toolCategory(tool) : 'tool', model, status: canceled ? 'error' : 'pending',
                attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'codewhale.call_id': callId, reason: payload.reason },
                payload: { arguments: clip(payload.arguments) }, raw: rec,
            });
            continue;
        }
        if (eventName === 'item.started' || eventName === 'item.completed') {
            const kind = str(item.kind) ?? 'item';
            const tool = itemToolName(item, payload);
            const id = itemId ?? `item:${rec.seq}`;
            const startWall = parseTime(item.started_at);
            const endWall = parseTime(item.ended_at);
            const start = startWall !== undefined && origin !== undefined ? startWall - origin : relative;
            const end = eventName === 'item.completed' && endWall !== undefined && origin !== undefined ? endWall - origin : relative;
            const openEnded = eventName === 'item.started' && endWall === undefined;
            const existing = open.get(id);
            if (existing && eventName === 'item.completed') {
                const prior = events[existing.index];
                prior.endTime = Math.max(prior.startTime, end);
                prior.openEnded = false;
                prior.status = statusOf(item.status);
                if (prior.status === 'error')
                    prior.attributes['whalesong.error_onset_ms'] = relative;
                prior.payload = { summary: clip(item.summary), detail: clip(item.detail) };
                open.delete(id);
                continue;
            }
            const event = {
                schemaVersion: 1, id, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
                startTime: start, endTime: Math.max(start, end), openEnded,
                agentId, name: tool ?? kind, tool, category: itemCategory(kind, tool), model,
                status: statusOf(item.status ?? (eventName === 'item.started' ? 'running' : undefined)),
                attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'codewhale.item_kind': kind,
                    ...(statusOf(item.status) === 'error' ? { 'whalesong.error_onset_ms': relative } : {}) },
                payload: { summary: clip(item.summary), detail: clip(item.detail) }, raw: rec,
            };
            if (eventName === 'item.started')
                open.set(id, { index: events.length, startWall: (origin ?? 0) + start });
            pushEvent(events, event);
            continue;
        }
        pushEvent(events, {
            schemaVersion: 1, id: `${eventName}:${rec.seq}`, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
            startTime: relative, endTime: relative, agentId, name: eventName, category: classify(eventName),
            model, status: 'unknown', attributes: { 'codewhale.seq': rec.seq }, raw: rec,
        });
    }
    if (skippedDeltas)
        warnings.push(`Dropped ${skippedDeltas.toLocaleString()} item.delta records; they are token stream fragments, not spans. Item start/end remain the source of duration.`);
    for (const [id] of open)
        warnings.push(`Item ${id} started and never completed in this file; duration remains unknown.`);
    for (const request of requests.values())
        warnings.push(`Request ${request.id} has no terminal receipt; its duration remains unknown in this file.`);
    if (!events.length)
        throw new Error('Codewhale runtime file contained only stream deltas or unreadable records.');
    const base = events.reduce((m, e) => Math.min(m, e.startTime), events[0].startTime);
    for (const event of events) {
        if (event.attributes['whalesong.error_onset_ms'] !== undefined)
            event.attributes['whalesong.error_onset_ms'] = (0, model_js_1.errorOnsetOf)(event) - base;
        event.startTime -= base;
        event.endTime -= base;
    }
    return {
        id: threadId,
        name: `Codewhale runtime · ${threadName}`,
        events,
        duration: Math.max(1, events.reduce((m, e) => Math.max(m, e.endTime, e.startTime, e.status === 'error' ? (0, model_js_1.errorOnsetOf)(e) : 0), 0)),
        originTime: origin !== undefined ? new Date(origin + base).toISOString() : '0 ms',
        source: 'codewhale',
        privacy: 'redact',
        warnings: [...new Set(warnings)],
        metadata: {
            sourceFormat: 'codewhale.runtime-events/v2',
            timeBasis: 'wall-clock',
            sourceFilename: filename,
            threadId,
            model,
            skippedDeltas,
            recordCount: records.length,
            timeUnit: 'ms',
        },
    };
}
/** The journal owns request state until a matching terminal receipt. A live
 * driver may confirm that state only while its cursor-checked stream is healthy.
 * Ordinary open tool spans remain unknown-duration; no execution is inferred. */
function observeRuntimeRequests(trace, observedThrough) {
    const origin = Date.parse(trace.originTime ?? '');
    if (trace.metadata.sourceFormat !== 'codewhale.runtime-events/v2' || !Number.isFinite(origin)
        || !Number.isFinite(observedThrough))
        throw new Error('Invalid Runtime observation horizon.');
    const at = observedThrough - origin;
    const events = trace.events.map(e => e.openEnded && e.attributes['whalesong.waiting'] === true && at >= e.startTime
        ? { ...e, endTime: at, openEnded: false } : e);
    return { ...trace, events, duration: Math.max(trace.duration, at) };
}

};
function load(id){id=id.replace(/^\.\//,'').replace(/\.js$/,'');if(cache[id])return cache[id];if(!factories[id])throw Error('Missing core module');const e=cache[id]={};factories[id](e,load);return e;}
global.PetNative=load('pet-native').PetNative;
})(globalThis);

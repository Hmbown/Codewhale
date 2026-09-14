import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { create } from '../src/backends/darwin.mjs';

function fixture(t, options = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-ocr-'));
  const saved = [process.env.CODEWHALE_CU_APP_BUNDLE, process.env.CODEWHALE_CU_RECORDINGS_DIR];
  t.after(() => {
    for (const [index, name] of ['CODEWHALE_CU_APP_BUNDLE', 'CODEWHALE_CU_RECORDINGS_DIR'].entries()) {
      if (saved[index] === undefined) delete process.env[name]; else process.env[name] = saved[index];
    }
    fs.rmSync(root, { recursive: true, force: true });
  });
  fs.mkdirSync(path.join(root, 'Contents', 'MacOS'), { recursive: true });
  fs.writeFileSync(path.join(root, 'Contents', 'MacOS', 'accessibility'), '');
  process.env.CODEWHALE_CU_APP_BUNDLE = root;
  process.env.CODEWHALE_CU_RECORDINGS_DIR = path.join(root, 'captures');
  const observed = { found: true, pid: 731, bundle_id: 'test.ocr', elements: [{ index: 0, role: 'AXButton', label: 'Save', actions: ['AXPress'], windowIndex: 1, path: [0] }] };
  if (options.missingIdentity) delete observed.pid;
  const calls = [];
  const backend = create({ exec: { async run(cmd, args) {
    if (cmd === 'screencapture') {
      calls.push({ tool: 'screencapture', args });
      if (options.captureFailure) return { code: 1, stderr: 'Screen Recording permission denied', stdout: '' };
      // A real PNG signature and IHDR: the backend identifies the raster
      // format before trusting its dimensions, so a headerless stub is not a
      // faithful stand-in for what screencapture writes.
      const header = Buffer.alloc(24);
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(header, 0);
      header.writeUInt32BE(13, 8); header.write('IHDR', 12, 'ascii');
      header.writeUInt32BE(200, 16); header.writeUInt32BE(100, 20);
      fs.writeFileSync(args.at(-1), header);
      return { code: 0, stdout: '', stderr: '' };
    }
    const request = JSON.parse(args[0]); calls.push(request);
    let body;
    if (request.tool === 'get_app_state') body = observed;
    else if (request.tool === 'input_capabilities') body = { window_ocr: options.oldHelper ? 0 : 1 };
    else if (request.tool === 'window_info') body = { window_id: 901, points: { x: -400, y: 200, w: 100, h: 50 } };
    else if (request.tool === 'displays') body = [{ index: 1, scale: 2 }];
    else if (request.tool === 'recognize_text') {
      if (options.cancelled) return { code: -1, stdout: '', stderr: '', aborted: true };
      body = options.ocr ?? { status: 'ok', engine: 'apple_vision', coordinate_space: 'raster_pixels', pixels: { w: 200, h: 100 }, blocks: [{ text: 'Invoice total', confidence: 0.92, bounds: { x: 20, y: 10, w: 40, h: 20 } }] };
    } else assert.fail(`unexpected native tool ${request.tool}`);
    return { code: 0, stdout: JSON.stringify(body), stderr: '' };
  } } });
  return { backend, calls, observed, root };
}

test('default app state remains AX-only without capture or OCR work', async t => {
  const { backend, calls, observed } = fixture(t);
  assert.deepEqual(await backend.get_app_state({ app_ref: { pid: 731 } }), observed);
  assert.deepEqual(calls.map(call => call.tool), ['get_app_state']);
});

test('opt-in OCR captures the resolved selected window and exposes raster-pixel text targets', async t => {
  const { backend, calls, observed } = fixture(t);
  const result = await backend.get_app_state({ app_ref: { name: 'Test' }, window_id: 1, include_ocr: true });
  assert.deepEqual(result.elements, observed.elements, 'AX roles and identity remain unchanged');
  const window = calls.find(call => call.tool === 'window_info');
  assert.deepEqual(window.args.app_ref, { pid: 731, bundle_id: 'test.ocr' });
  assert.equal(window.args.window_id, 1);
  const capture = calls.find(call => call.tool === 'screencapture');
  assert.deepEqual(capture.args.slice(0, -1), ['-x', '-t', 'png', '-o', '-l', '901']);
  assert.equal(result.ocr.status, 'ok');
  assert.deepEqual(result.ocr.blocks[0].target, { type: 'coordinate', x: 40, y: 20 });
  assert.equal(result.ocr.blocks[0].role, undefined, 'recognized text is not an accessibility control');
  assert.deepEqual(result.ocr.raster.points, { x: -400, y: 200, w: 100, h: 50 });
  assert.equal(result.ocr.raster.scale, 2);
  assert.equal(calls.find(call => call.tool === 'recognize_text').args.file, result.ocr.raster.file);
});

test('capture permission failure preserves valid AX state with an explicit OCR diagnosis', async t => {
  const { backend, calls, observed } = fixture(t, { captureFailure: true });
  const result = await backend.get_app_state({ app_ref: { pid: 731 }, include_ocr: true });
  assert.deepEqual(result.elements, observed.elements);
  assert.equal(result.ocr.status, 'unavailable');
  assert.match(result.ocr.reason, /Screen Recording permission denied/);
  assert.equal(result.ocr.raster, undefined);
  assert.ok(!calls.some(call => call.tool === 'recognize_text'));
});

test('OCR failure keeps AX state and captured geometry but never invents text targets', async t => {
  const { backend, observed } = fixture(t, { ocr: { status: 'unavailable', reason: 'Apple Vision unavailable' } });
  const result = await backend.get_app_state({ app_ref: { pid: 731 }, include_ocr: true });
  assert.deepEqual(result.elements, observed.elements);
  assert.equal(result.ocr.reason, 'Apple Vision unavailable');
  assert.deepEqual(result.ocr.blocks, []);
  assert.deepEqual(result.ocr.raster.pixels, { w: 200, h: 100 }, 'MCP can bind the new capture even when OCR fails');
});

test('OCR image dimension mismatch fails without discarding AX state', async t => {
  const { backend, observed } = fixture(t, { ocr: { status: 'ok', pixels: { w: 400, h: 200 }, blocks: [] } });
  const result = await backend.get_app_state({ app_ref: { pid: 731 }, include_ocr: true });
  assert.deepEqual(result.elements, observed.elements);
  assert.equal(result.ocr.status, 'unavailable');
  assert.match(result.ocr.reason, /mismatched image dimensions/);
});

test('missing resolved identity cannot redirect optional OCR to the foreground application', async t => {
  const { backend, calls } = fixture(t, { missingIdentity: true });
  const result = await backend.get_app_state({ app_ref: { name: 'Test' }, include_ocr: true });
  assert.equal(result.ocr.status, 'unavailable');
  assert.match(result.ocr.reason, /exact process identity/);
  assert.deepEqual(calls.map(call => call.tool), ['get_app_state']);
});

test('an older helper is refused before it can capture a different window', async t => {
  const { backend, calls } = fixture(t, { oldHelper: true });
  const result = await backend.get_app_state({ app_ref: { pid: 731 }, window_id: 1, include_ocr: true });
  assert.equal(result.ocr.status, 'unavailable');
  assert.match(result.ocr.reason, /helper needs an update/);
  assert.deepEqual(calls.map(call => call.tool), ['get_app_state', 'input_capabilities']);
});

test('OCR cancellation remains cancellation', async t => {
  const { backend } = fixture(t, { cancelled: true });
  await assert.rejects(backend.get_app_state({ app_ref: { pid: 731 }, include_ocr: true }), error => error.code === 'cancelled');
});

test('Apple Vision recognizes a generated image with correct raster bounds and rejects invalid images', { skip: process.platform !== 'darwin' }, t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-vision-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, 'probe.m'), binary = path.join(root, 'probe'), image = path.join(root, 'text.png');
  fs.writeFileSync(source, `#import ${JSON.stringify(path.resolve('src/backends/darwin-ocr.h'))}
int main(int argc,const char **argv) { @autoreleasepool {
  NSString *file=[NSString stringWithUTF8String:argv[1]];
  NSBitmapImageRep *bitmap=[[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:1000 pixelsHigh:320 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
  [NSGraphicsContext saveGraphicsState];
  [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap]];
  [NSColor.whiteColor setFill]; NSRectFill(NSMakeRect(0,0,1000,320));
  [@"Codewhale OCR 7319" drawAtPoint:NSMakePoint(50,220) withAttributes:@{NSFontAttributeName:[NSFont systemFontOfSize:48],NSForegroundColorAttributeName:NSColor.blackColor}];
  [@"Invoice total 42.50" drawAtPoint:NSMakePoint(50,70) withAttributes:@{NSFontAttributeName:[NSFont systemFontOfSize:40],NSForegroundColorAttributeName:NSColor.blackColor}];
  [NSGraphicsContext restoreGraphicsState];
  [[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:file atomically:YES];
  NSDictionary *recognized=cuRecognizeText(file);
  [@"invalid image" writeToFile:file atomically:YES encoding:NSUTF8StringEncoding error:nil];
  NSDictionary *result=@{@"recognized":recognized,@"invalid":cuRecognizeText(file),@"missing":cuRecognizeText([file stringByAppendingString:@".missing"]),@"bounds":cuOCRPixelBounds(CGRectMake(.125,.25,.25,.5),1000,800),@"clipped":cuOCRPixelBounds(CGRectMake(-.5,-.5,2,2),1000,800)};
  NSData *json=[NSJSONSerialization dataWithJSONObject:result options:0 error:nil];
  puts([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
} return 0; }`);
  const build = spawnSync('clang', ['-fobjc-arc', '-Os', '-framework', 'Cocoa', '-framework', 'Vision', source, '-o', binary], { encoding: 'utf8' });
  assert.equal(build.status, 0, build.stderr);
  const run = spawnSync(binary, [image], { encoding: 'utf8', timeout: 20000 });
  assert.equal(run.status, 0, run.stderr);
  const result = JSON.parse(run.stdout);
  assert.equal(result.recognized.status, 'ok', JSON.stringify(result.recognized));
  const first = result.recognized.blocks.find(block => block.text === 'Codewhale OCR 7319');
  const second = result.recognized.blocks.find(block => block.text === 'Invoice total 42.50');
  assert.ok(first && second, JSON.stringify(result.recognized));
  assert.ok(first.confidence > 0.8 && second.confidence > 0.8);
  assert.ok(first.bounds.x >= 40 && first.bounds.x < 65 && first.bounds.y >= 40 && first.bounds.y < 100);
  assert.ok(second.bounds.y > 200 && second.bounds.y < 260, 'Vision lower-left coordinates became top-left raster coordinates');
  assert.deepEqual(result.bounds, { x: 125, y: 200, w: 250, h: 400 });
  assert.deepEqual(result.clipped, { x: 0, y: 0, w: 1000, h: 800 });
  assert.equal(result.invalid.status, 'unavailable');
  assert.equal(result.missing.status, 'unavailable');
});

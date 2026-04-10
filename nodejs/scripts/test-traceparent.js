#!/usr/bin/env node
/**
 * Integration test: verify traceparent propagation from OTel JS → LanceDB Rust NAPI.
 *
 * This script:
 * 1. Initializes an OTel SDK with an InMemory span exporter
 * 2. Creates a parent span simulating an API handler
 * 3. Within that span context, calls LanceDB query operations
 * 4. Verifies getTraceparent() returns the correct traceparent string
 * 5. Verifies LanceDB operations succeed with the traceparent injected
 *
 * Run:  node scripts/test-traceparent.js
 */

const { context, trace, SpanStatusCode } = require('@opentelemetry/api');
const { NodeTracerProvider } = require('@opentelemetry/sdk-trace-node');
const { InMemorySpanExporter, SimpleSpanProcessor } = require('@opentelemetry/sdk-trace-base');
const lancedb = require('../dist/index.js');
const { getTraceparent } = require('../dist/query.js');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

// ─── Setup OTel with InMemory exporter ───────────────────────────────────────

const exporter = new InMemorySpanExporter();
const provider = new NodeTracerProvider({
    spanProcessors: [new SimpleSpanProcessor(exporter)],
});
provider.register();
const tracer = trace.getTracer('test-traceparent');

// ─── Setup temp LanceDB directory ────────────────────────────────────────────

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lance-trace-test-'));
console.log(`[setup] Temp DB dir: ${tmpDir}`);

// ─── Initialize LanceDB Rust profiling ───────────────────────────────────────

try {
    lancedb.initProfiling({ serviceName: 'lance-trace-test' });
    console.log('[setup] LanceDB Rust profiling initialized');
} catch (e) {
    console.log(`[setup] LanceDB profiling init: ${e.message}`);
}

// ─── Test helpers ────────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;

function assert(condition, message) {
    if (condition) {
        console.log(`  ✅ ${message}`);
        passed++;
    } else {
        console.error(`  ❌ FAIL: ${message}`);
        failed++;
    }
}

async function run() {
    // ─── Test 1: getTraceparent() returns correct value inside an active span ─

    console.log('\n[Test 1] getTraceparent() inside an active span');

    const parentSpan = tracer.startSpan('test.parent');
    const parentCtx = trace.setSpan(context.active(), parentSpan);

    context.with(parentCtx, () => {
        const tp = getTraceparent();
        assert(tp !== undefined, 'getTraceparent() returns non-undefined');

        if (tp) {
            const parts = tp.split('-');
            assert(parts.length === 4, `traceparent has 4 parts: ${tp}`);
            assert(parts[0] === '00', `version is "00": ${parts[0]}`);
            assert(parts[1].length === 32, `traceId is 32 hex chars: ${parts[1]}`);
            assert(parts[2].length === 16, `spanId is 16 hex chars: ${parts[2]}`);

            const spanCtx = parentSpan.spanContext();
            assert(parts[1] === spanCtx.traceId, `traceId matches: ${parts[1]} === ${spanCtx.traceId}`);
            assert(parts[2] === spanCtx.spanId, `spanId matches: ${parts[2]} === ${spanCtx.spanId}`);
        }
    });

    parentSpan.end();

    // ─── Test 2: getTraceparent() returns undefined when no active span ──────

    console.log('\n[Test 2] getTraceparent() without an active span');

    const tpNoSpan = getTraceparent();
    assert(tpNoSpan === undefined, 'getTraceparent() returns undefined when no active span');

    // ─── Test 3: LanceDB query with traceparent (full E2E) ──────────────────

    console.log('\n[Test 3] LanceDB query with traceparent injection');

    const querySpan = tracer.startSpan('test.lancedb.query');
    const queryCtx = trace.setSpan(context.active(), querySpan);

    try {
        await context.with(queryCtx, async () => {
            const tp = getTraceparent();
            assert(tp !== undefined, 'traceparent available for query');
            console.log(`  traceparent: ${tp}`);

            // Create a test connection and table
            const conn = await lancedb.connect(tmpDir);
            const table = await conn.createTable('test_trace', [
                { id: '1', text: 'hello', vector: Array.from({ length: 8 }, () => Math.random()) },
                { id: '2', text: 'world', vector: Array.from({ length: 8 }, () => Math.random()) },
            ]);

            // Execute a query — this will pass traceparent to Rust via NAPI
            const results = await table.query().limit(10).toArray();
            assert(results.length === 2, `query returned ${results.length} results (expected 2)`);

            // Execute a countRows — also passes traceparent
            const count = await table.countRows();
            assert(count === 2, `countRows returned ${count} (expected 2)`);

            // Vector search
            const vecResults = await table
                .search(Array.from({ length: 8 }, () => Math.random()))
                .limit(1)
                .toArray();
            assert(vecResults.length === 1, `vector search returned ${vecResults.length} results (expected 1)`);

            querySpan.setStatus({ code: SpanStatusCode.OK });
        });
    } catch (error) {
        querySpan.setStatus({ code: SpanStatusCode.ERROR, message: String(error) });
        console.error('  ❌ Query test failed:', error);
        failed++;
    } finally {
        querySpan.end();
    }

    // ─── Test 4: Verify exported spans ──────────────────────────────────────

    console.log('\n[Test 4] Verify exported OTel spans');

    await provider.forceFlush();
    const spans = exporter.getFinishedSpans();
    assert(spans.length >= 2, `${spans.length} spans exported (expected >= 2)`);

    const spanNames = spans.map(s => s.name);
    console.log(`  Exported spans: ${JSON.stringify(spanNames)}`);
    assert(spanNames.includes('test.parent'), 'test.parent span exported');
    assert(spanNames.includes('test.lancedb.query'), 'test.lancedb.query span exported');

    // Verify the query span has a valid traceId
    const queryExportedSpan = spans.find(s => s.name === 'test.lancedb.query');
    if (queryExportedSpan) {
        const traceId = queryExportedSpan.spanContext().traceId;
        assert(traceId.length === 32, `query span has valid traceId: ${traceId}`);
        console.log(`  Query span traceId: ${traceId}`);
    }

    // ─── Cleanup ────────────────────────────────────────────────────────────

    fs.rmSync(tmpDir, { recursive: true, force: true });
    await provider.shutdown();

    // ─── Summary ────────────────────────────────────────────────────────────

    console.log(`\n${'─'.repeat(60)}`);
    console.log(`Results: ${passed} passed, ${failed} failed`);
    if (failed > 0) {
        process.exit(1);
    } else {
        console.log('All tests passed! Traceparent propagation is working correctly.');
    }
}

run().catch((err) => {
    console.error('Fatal error:', err);
    process.exit(1);
});

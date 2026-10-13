from pathlib import Path
import hashlib, json, math
root = Path('/root/daily-archive')
base = root / 'artifacts/learning/policy-corpus-m517'
def load(path):
    return json.loads(path.read_text())
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def artifact(directory, item):
    assert item is not None
    path = directory / item['path']
    assert digest(path) == item['sha256'], str(path)
    return path
manifest_path = base / 'manifest.json'
manifest = load(manifest_path)
docs = manifest['documents']
assert len(docs) == 320
assert len({d['source_sha256'] for d in docs}) == 320
assert len({d['paper_id'] for d in docs}) == 320
train = [d for d in docs if d['partition'] == 'train']
validation = [d for d in docs if d['partition'] == 'validation']
assert len(train) == 240 and len(validation) == 80
assert not {d['paper_id'] for d in train} & {d['paper_id'] for d in validation}
capture_prov = load(base / 'capture-provenance.json')
eval_prov = load(base / 'evaluator-provenance.json')
assert digest(manifest_path) == capture_prov['manifest_sha256'] == eval_prov['manifest_sha256']
assert digest(base / 'experiment.yaml') == capture_prov['config_sha256'] == eval_prov['experiment_sha256']
assert digest(base / 'registry.yaml') == capture_prov['registry_sha256'] == eval_prov['native_registry_sha256']
assert digest(Path('/tmp/policy-resume-audit/frozen-runners/policy_corpus_capture')) == capture_prov['capture_executable_sha256']
assert digest(Path('/tmp/policy-resume-audit/frozen-runners/policy_corpus_evaluate')) == eval_prov['executable_sha256']
for relative, expected in eval_prov['sources'].items():
    assert digest(root / relative) == expected, relative
capture_statuses = {}
for doc in docs:
    assert digest(Path(doc['pdf_path'])) == doc['source_sha256']
    directory = root / doc['capture_dir']
    capture = load(directory / 'capture.json')
    assert capture['source']['sha256'] == doc['source_sha256']
    assert capture['paper_id'] == doc['paper_id']
    assert capture['features'] is not None and capture['features_error'] is None
    assert capture['reference']['error'] is None
    assert capture['candidate_only'] and not capture['deployed'] and not capture['import_eligible'] and not capture['graph_write']
    artifact(directory, capture['reference']['artifact'])
    if capture['exporter']['observations'] is not None:
        artifact(directory, capture['exporter']['observations'])
    for arm in capture['arms']:
        capture_statuses[arm['arm'] + ':' + arm['status']] = capture_statuses.get(arm['arm'] + ':' + arm['status'], 0) + 1
        assert arm['latency_ms'] >= 0
        if arm['status'] == 'success':
            artifact(directory, arm['article'])
            assert arm['evidence']['ok'] is True
        elif arm['status'] == 'not_applicable':
            assert arm['arm'] == 'toc' and arm['error'] == 'empty_toc'
        else:
            assert arm['status'] == 'failed' and arm['error']
        if arm['arm'] == 'grobid':
            assert arm['backend_profile'] == 'structural' and arm['backend_version'] == '0.9.1'
    assert capture['exporter']['script']['sha256'] == digest(Path(capture['exporter']['script']['path']))
    assert capture['exporter']['furniture_policy']['sha256'] == digest(Path(capture['exporter']['furniture_policy']['path']))
historical = manifest['historical']
for item in historical:
    assert digest(Path(item['original_path'])) == item['sha256']
    assert digest(root / item['copied_path']) == item['sha256']
old_ids = {json.loads(line)['document_identity'] for line in (root / historical[1]['copied_path']).read_text().splitlines() if line.strip()}
assert not {d['paper_id'] for d in docs} & old_ids
reports = [load(base / f'cycle-{limit:03d}.json') for limit in (60, 120, 240)]
previous = None
summary = []
for limit, report in zip((60, 120, 240), reports):
    assert report['train_limit'] == limit
    assert report['train_source_sha256'] == [d['source_sha256'] for d in train[:limit]]
    assert report['validation_source_sha256'] == [d['source_sha256'] for d in validation]
    assert report['manifest_sha256'] == digest(manifest_path)
    assert report['config_sha256'] == digest(base / 'experiment.yaml')
    assert report['native_registry_sha256'] == digest(base / 'registry.yaml')
    assert report['candidate_only'] and not report['deployed'] and not report['import_eligible']
    assert report['auto_promote'] == 'forbidden' and report['graph_write'] == 'forbidden'
    assert report['validation_usage'] == 'adaptive_diagnostic_not_final_test'
    assert len(report['metrics']) == 8
    for name, metric in report['metrics'].items():
        assert digest(Path(metric['table']['path'])) == metric['table']['output_sha256']
        rows = metric['validation_documents']
        assert len(rows) == 80
        assert [r['source_sha256'] for r in rows] == report['validation_source_sha256']
        for label in ('native', 'candidate'):
            choices = [r[label] for r in rows]
            agg = metric['validation'][label]
            assert agg['n'] == 80
            rewards = []
            for row, choice in zip(rows, choices):
                pair = row['pair']
                if not choice['outcome_ok']:
                    expected = 0.0
                elif pair['kind'] == 'tie':
                    expected = 0.5
                elif pair['winner'] == choice['execution_arm']:
                    expected = 1.0
                else:
                    expected = 0.0
                assert choice['reward'] == expected
                rewards.append(expected)
                directory = root / row['provenance']['capture_dir']
                assert digest(directory / 'capture.json') == row['capture_sha256']
                assert row['reference_sha256'] == load(directory / 'capture.json')['reference']['artifact']['sha256']
            assert agg['selected_reward']['n'] == 80
            assert math.isclose(agg['selected_reward']['mean'], sum(rewards) / 80, abs_tol=1e-12)
            assert agg['operational_success']['count'] == sum(c['outcome_ok'] for c in choices)
            assert math.isclose(agg['mean_selected_latency_ms']['mean'],sum(c['latency_ms'] for c in choices)/80, abs_tol=1e-9)
            for axis, aggregate in [('token_overlap','successful_selected_token_f1'),('ordered_5gram','successful_selected_ordered_5gram_f1')]:
                values = [c['text_metrics'][axis]['f1'] for c in choices if c['outcome_ok'] and c['text_metrics'] is not None]
                assert agg[aggregate]['n'] == len(values)
                assert all(0 <= value <= 1 for value in values)
                if values:
                    assert math.isclose(agg[aggregate]['mean'],sum(values)/len(values), abs_tol=1e-12)
            decisive = [r for r in rows if r['pair']['kind'] == 'decisive']
            assert agg['full_pair_decisive_accuracy']['n'] == len(decisive)
            assert agg['full_pair_decisive_accuracy']['correct'] == sum(r[label]['execution_arm'] == r['pair']['winner'] for r in decisive)
        if previous:
            assert report['evaluator_source_sha256'] == previous['evaluator_source_sha256']
            old_rows = previous['metrics'][name]['validation_documents']
            for old, new in zip(old_rows, rows):
                for key in ('source_sha256', 'reference_sha256', 'capture_sha256', 'features', 'pair', 'native', 'arms'):
                    assert old[key] == new[key], (limit, name, key)
        summary.append({'train_limit':limit,'metric':name,'validation_n':80,'native_reward':metric['validation']['native']['selected_reward']['mean'],'candidate_reward':metric['validation']['candidate']['selected_reward']['mean']})
    previous = report

# Independent text-score oracle: Python regex and Counter, not Rust evaluator calls.
import re
from collections import Counter
def tokenize(text):
    return [word.lower() for word in re.findall(r'[A-Za-z]+', text, flags=re.ASCII)]
def score(candidate, reference):
    ca, ra = Counter(candidate), Counter(reference)
    matches = sum(min(n, ra.get(unit, 0)) for unit, n in ca.items())
    denominator = len(candidate) + len(reference)
    return {'candidate_units':len(candidate),'reference_units':len(reference),'matches':matches,'precision':matches/len(candidate) if candidate else None,'recall':matches/len(reference) if reference else None,'f1':2*matches/denominator if denominator else None}
def verify_score(expected, actual):
    for key, value in expected.items():
        if isinstance(value, float):
            assert math.isclose(value,actual[key],abs_tol=1e-12), (key,value,actual[key])
        else:
            assert value == actual[key], (key,value,actual[key])
metric_rows = {r['source_sha256']:r for r in reports[-1]['metrics']['token_f1']['validation_documents']}
# Report arms are identical across metric variants and cycle sizes; validate every scored validation arm.
for doc in validation:
    directory = root / doc['capture_dir']
    cap = load(directory / 'capture.json')
    reference = tokenize((directory / cap['reference']['artifact']['path']).read_text())
    row = metric_rows[doc['source_sha256']]
    for reported in row['arms']:
        arm = reported['capture']
        if arm['status'] != 'success':
            assert reported['metrics'] is None
            continue
        article = load(directory / arm['article']['path'])
        text = '\n'.join(section['text'] for section in article['sections'])
        candidate = tokenize(text)
        verify_score(score(candidate,reference),reported['metrics']['token_overlap'])
        candidate_grams = [tuple(candidate[i:i+5]) for i in range(max(0,len(candidate)-4))]
        reference_grams = [tuple(reference[i:i+5]) for i in range(max(0,len(reference)-4))]
        verify_score(score(candidate_grams,reference_grams),reported['metrics']['ordered_5gram'])
        assert reported['metrics']['body_chars'] == sum(len(s['text']) for s in article['sections'])
        assert reported['metrics']['sections'] == len(article['sections'])
# Historical journals and production-artifact tree stats are unchanged.
baseline = load(base / 'production-baseline.json')
for relative, before in baseline['tracked_directories'].items():
    after = {str(p.relative_to(root)):[p.stat().st_size,p.stat().st_mtime_ns] for p in (root / relative).rglob('*') if p.is_file()}
    assert after == before, relative
for item in baseline['historical_files']:
    assert digest(Path(item['original_path'])) == item['sha256']
    assert digest(root / item['copied_path']) == item['sha256']

print(json.dumps({'verdict':'PASS','kind':'retained-artifact-integrity-and-independent-aggregation','captures':320,'source_family_ids':320,'fixed_validation_n':80,'training_prefixes':[60,120,240],'capture_statuses':capture_statuses,'cycle_summaries':summary}, indent=2))

# All final current-source repairs are separate from the frozen measurement recipe.
current_seal = load(base / 'verification/current-source-verification.json')
for relative, expected in current_seal['current_source_sha256'].items():
    assert digest(root / relative) == expected, ('current_source',relative)
report = load(root / 'doc/review/policy-corpus-cycles.json')
for entry in report['reports']:
    assert digest(root / entry['path']) == entry['sha256']
    assert digest(root / entry['diagnostic_path']) == entry['diagnostic_sha256']
for key in ['post_measurement_lint_repair', 'post_measurement_contract_inventory', 'post_measurement_timeout_test', 'independent_review']:
    assert digest(root / report[key]['path']) == report[key]['sha256'], key
print('PASS current-source seal, frozen runner identities, post-measurement records and final report references')

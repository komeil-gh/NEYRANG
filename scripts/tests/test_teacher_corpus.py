import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from scripts import teacher_corpus
from scripts.teacher_export import export_shard
from scripts.tests.test_teacher_export import fixture


def exported(root, name, moves, fen):
    source, game, rows = fixture(moves, fen)
    inputs = root / (name + '-raw')
    inputs.mkdir()
    payloads = {'groups.json': json.dumps([source]), 'games.jsonl': json.dumps(game) + '\n',
                'labels.jsonl': ''.join(json.dumps(r) + '\n' for r in rows),
                'protocol.log': 'Synthetic test only; no teacher evidence.\n'}
    for n, contents in payloads.items():
        (inputs / n).write_text(contents)
    audit = {'schema': 'neyrang-teacher-source-audit-v1', 'passed': True, 'partition': 'train',
             'max_plies': max(4, len(rows)), 'max_nodes': 1100, 'teacher_sha256': '0' * 64,
             'sha256': {n: hashlib.sha256(s.encode()).hexdigest() for n, s in payloads.items()}}
    (inputs / 'source-audit.json').write_text(json.dumps(audit))
    out = root / name
    export_shard(inputs, out, 'train', max(4, len(rows)), 1100)
    return out, source, rows


class TeacherCorpusTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        # Distinct roots share later positions; first registered occurrence wins.
        fen = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
        a, sa, ra = exported(self.root, 'a', 'f2f3 e7e5 g2g4 d8h4', fen)
        fen2 = 'rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq - 0 1'
        b, sb, rb = exported(self.root, 'b', 'e7e5 g2g4 d8h4', fen2)
        self.inputs, self.sources, self.rows = [a, b], [sa, sb], [ra, rb]
        self.groups = self.root / 'groups.json'
        self.groups.write_text(json.dumps(self.sources))

    def assemble(self, name='out', count=3, exposures=2):
        return teacher_corpus.assemble(self.inputs, self.groups, self.root / name,
                                       count, exposures, 'test-ranking')

    def test_registered_order_dedup_ranking_and_exact_exposures(self):
        report = self.assemble()
        seen, expected = set(), []
        for rows in self.rows:
            for row in rows:
                key = ' '.join(row['fen'].split()[:4])
                if key in seen:
                    continue
                seen.add(key)
                rank = hashlib.sha256(f"test-ranking\t{row['group']}\t{row['ply']}".encode()).hexdigest()
                expected.append((rank, row['group'], row['ply']))
        expected = sorted(expected)[:3]
        chosen = [json.loads(x) for x in (self.root / 'out/selected.jsonl').read_text().splitlines()]
        self.assertEqual([(r['rank'], r['group'], r['ply']) for r in chosen], expected)
        base = (self.root / 'out/base.txt').read_text().splitlines()
        repeated = (self.root / 'out/train.txt').read_text().splitlines()
        self.assertEqual(repeated, [base[0]] + base[1:] * 2)
        self.assertEqual(report['positions'], 6)
        self.assertEqual(report['all_position_keys'], len(seen))
        self.assertEqual(report['eligible_unique'], len(seen))
        self.assertEqual(len(seen), 4)
        self.assertTrue(all(r['group'] == self.sources[0]['group'] for r in chosen))
        self.assertEqual(len((self.root / 'out/position-keys.txt').read_text().splitlines()), len(seen))
        second = self.assemble('second')
        self.assertEqual(report['sha256'], second['sha256'])
        with self.assertRaises(FileExistsError):
            self.assemble()

    def test_quota_order_partition_and_corruption_fail_without_success(self):
        with self.assertRaisesRegex(ValueError, 'quota'):
            self.assemble(count=100)
        self.assertFalse((self.root / 'out/manifest.json').exists())
        self.inputs.reverse()
        with self.assertRaisesRegex(ValueError, 'order'):
            self.assemble('order')
        self.inputs.reverse()
        manifest = self.inputs[0] / 'manifest.json'
        original = manifest.read_text()
        changed = json.loads(original)
        changed['partition'] = 'validation'
        manifest.write_text(json.dumps(changed))
        with self.assertRaisesRegex(ValueError, 'partition'):
            self.assemble('partition')
        manifest.write_text(original)
        with (self.inputs[0] / 'train.txt').open('a') as stream:
            stream.write('corruption\n')
        with self.assertRaisesRegex(ValueError, 'hash'):
            self.assemble('corrupt')

    def test_provenance_and_export_disagreement_is_rejected(self):
        path = self.inputs[0] / 'provenance.jsonl'
        item = json.loads(path.read_text())
        item['rows'][0]['wdl_white'] = [500, 0, 500]
        path.write_text(json.dumps(item) + '\n')
        manifest = self.inputs[0] / 'manifest.json'
        doc = json.loads(manifest.read_text())
        doc['sha256']['provenance.jsonl'] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest.write_text(json.dumps(doc))
        with self.assertRaisesRegex(ValueError, 'text/provenance'):
            self.assemble()

    def test_capped_game_rows_are_exclusion_keys_but_never_training_rows(self):
        source, game, rows = fixture('g1f3 g8f6 f3g1 f6g8', self.sources[0]['fen'])
        # Keep the completed second root distinct from the capped starting root.
        raw = self.root / 'b-raw'
        (raw / 'groups.json').write_text(json.dumps([self.sources[1], source]))
        with (raw / 'games.jsonl').open('a') as stream:
            stream.write(json.dumps(game) + '\n')
        with (raw / 'labels.jsonl').open('a') as stream:
            stream.writelines(json.dumps(r) + '\n' for r in rows)
        audit_path = raw / 'source-audit.json'
        audit = json.loads(audit_path.read_text())
        audit['sha256'] = {n: hashlib.sha256((raw / n).read_bytes()).hexdigest() for n in audit['sha256']}
        audit_path.write_text(json.dumps(audit))
        extra = self.root / 'with-cap'
        export_shard(raw, extra, 'train', 4, 1100)
        self.inputs = [extra]
        self.groups.write_text(json.dumps([self.sources[1], source]))
        result = self.assemble(count=3)
        self.assertEqual(result['eligible_unique'], 3)
        self.assertGreater(result['all_position_keys'], 3)
        chosen = [json.loads(x) for x in (self.root / 'out/selected.jsonl').read_text().splitlines()]
        self.assertTrue(all(r['group'] == self.sources[1]['group'] for r in chosen))


if __name__ == '__main__':
    unittest.main()

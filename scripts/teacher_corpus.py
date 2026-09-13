"""Select an exact, deterministic TRAIN multiset from audited teacher exports.

Consumes existing export attestations; does not replace raw-UCI audits. The
caller must bind the complete experiment's export list and registered settings.
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

from .teacher_export import require
from .teacher_target import ENCODING, wdl_score


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def assemble(inputs, groups_path, output, count, exposures, seed):
    inputs, groups_path, output = [Path(p) for p in inputs], Path(groups_path), Path(output)
    require(inputs and len({p.resolve() for p in inputs}) == len(inputs), 'duplicate/empty exports')
    require(type(count) is int and count > 0 and type(exposures) is int and exposures > 0
            and isinstance(seed, str) and seed, 'positive quota/exposures and seed required')
    if output.exists():
        raise FileExistsError(output)
    bindings = {groups_path: digest(groups_path)}
    groups = json.loads(groups_path.read_text())
    require(isinstance(groups, list) and groups and all(isinstance(g, dict) for g in groups),
            'missing registered groups')
    require(len({g['group'] for g in groups}) == len(groups), 'duplicate registered groups')
    manifests = []
    policy = None
    for path in inputs:
        manifest_path = path / 'manifest.json'
        bindings[manifest_path] = digest(manifest_path)
        manifest = json.loads(manifest_path.read_text())
        require(manifest.get('schema') == 'neyrang-teacher-export-v1'
                and manifest.get('encoding') == ENCODING and manifest.get('partition') == 'train',
                'export schema/encoding/partition mismatch')
        current = {k: manifest[k] for k in ('teacher_sha256', 'max_plies', 'max_nodes', 'tool_sha256')}
        require(policy is None or current == policy, 'mixed export policy')
        policy = current
        for name in ('train.txt', 'provenance.jsonl'):
            bindings[path / name] = digest(path / name)
            require(bindings[path / name] == manifest['sha256'][name], 'export hash mismatch')
        manifests.append(manifest)

    # One game of provenance at a time; never materialize the repeated corpus.
    all_keys, eligible = set(), {}
    group_index = 0
    for path, manifest in zip(inputs, manifests):
        exported_rows = completed = games = 0
        with (path / 'train.txt').open() as text, (path / 'provenance.jsonl').open() as provenance:
            require(text.readline() == '# ' + ENCODING + '\n', 'encoding header mismatch')
            while raw := provenance.readline(16 * 1024**2 + 1):
                require(len(raw) <= 16 * 1024**2, 'provenance game exceeds 16 MiB')
                item = json.loads(raw)
                require(group_index < len(groups) and item['source'] == groups[group_index],
                        'registered group order mismatch')
                source, game, rows = item['source'], item['game'], item['rows']
                require(item['partition'] == 'train' and game['group'] == source['group']
                        and game['fen'] == source['fen'] and isinstance(rows, list)
                        and 0 < len(rows) == game['plies'] <= manifest['max_plies'],
                        'provenance game/partition mismatch')
                require(game['result'] in (None, '1-0', '0-1', '1/2-1/2'), 'invalid game result')
                actual = {'1-0': '1.0', '0-1': '0.0', '1/2-1/2': '0.5', None: None}[game['result']]
                converted = 0
                for ply, row in enumerate(rows):
                    require(row['group'] == source['group'] and type(row['ply']) is int
                            and row['ply'] == ply and len(row['fen'].split()) == 6
                            and type(row['finite_quiet_candidate']) is bool,
                            'provenance row order/shape mismatch')
                    key = ' '.join(row['fen'].split()[:4])
                    all_keys.add(key)
                    if actual is None or not row['finite_quiet_candidate']:
                        continue
                    line = f"{row['fen']} | {wdl_score(row['wdl_white'])} | {actual}\n"
                    require(text.readline(257) == line, 'text/provenance mismatch')
                    require(len(line.encode('ascii')) <= 256, 'trainer line limit exceeded')
                    converted += 1
                    if key not in eligible:
                        group = row['group']
                        rank = hashlib.sha256(f'{seed}\t{group}\t{ply}'.encode()).hexdigest()
                        eligible[key] = (rank, group, ply, line)
                require(converted == item['exported_rows'], 'exported game count mismatch')
                exported_rows += converted
                completed += actual is not None
                games += 1
                group_index += 1
            require(text.read(1) == '', 'extra exported text')
        require((exported_rows, completed, games) ==
                (manifest['positions'], manifest['completed_games'], manifest['games']),
                'export totals mismatch')
    require(group_index == len(groups), 'incomplete registered group order')
    require(len(eligible) >= count, f'eligible unique quota failed: {len(eligible)} < {count}')
    selected = sorted(eligible.values())[:count]
    header = ('# ' + ENCODING + '\n').encode('ascii')
    base_bytes = sum(len(row[3].encode('ascii')) for row in selected)
    # The repeated text is streamed, so disk use is explicit before any output.
    needed = base_bytes * (1 + exposures) + sum(len(k) + 1 for k in all_keys) + count * 256
    require(shutil.disk_usage(output.parent).free >= needed + 1024**3, 'insufficient corpus disk headroom')
    require(all(digest(p) == h for p, h in bindings.items()), 'input hash changed during selection')
    output.mkdir()
    with (output / 'base.txt').open('xb') as base, (output / 'selected.jsonl').open('x') as ids:
        base.write(header)
        for rank, group, ply, line in selected:
            base.write(line.encode('ascii'))
            ids.write(json.dumps({'rank': rank, 'group': group, 'ply': ply}) + '\n')
    with (output / 'position-keys.txt').open('x') as keys:
        for key in sorted(all_keys):
            keys.write(key + '\n')
    with (output / 'train.txt').open('xb') as repeated, (output / 'base.txt').open('rb') as base:
        repeated.write(header)
        for _ in range(exposures):
            base.seek(len(header))
            shutil.copyfileobj(base, repeated, length=1024**2)
    require((output / 'train.txt').stat().st_size == len(header) + base_bytes * exposures,
            'repeated corpus byte count mismatch')
    names = ('base.txt', 'train.txt', 'selected.jsonl', 'position-keys.txt')
    report = {'schema': 'neyrang-teacher-corpus-v1', 'encoding': ENCODING, 'partition': 'train',
              'unique_selected': count, 'exposures': exposures, 'positions': count * exposures,
              'eligible_unique': len(eligible), 'all_position_keys': len(all_keys),
              'groups': group_index, 'ranking_seed': seed, 'policy': policy,
              'input_sha256': {str(p): h for p, h in bindings.items()},
              'tool_sha256': digest(__file__),
              'sha256': {n: digest(output / n) for n in names},
              'bytes': {n: (output / n).stat().st_size for n in names},
              'boundary': 'Existing export attestations, not a new source audit. All observed TRAIN '
                          'position keys, including ineligible/capped rows, are retained for evaluation '
                          'exclusions. Exposures repeat positions, not independent epochs or new data.'}
    with (output / 'manifest.json').open('x') as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write('\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, action='append', required=True)
    parser.add_argument('--groups', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--count', type=int, required=True)
    parser.add_argument('--exposures', type=int, required=True)
    parser.add_argument('--seed', required=True)
    args = parser.parse_args()
    report = assemble(args.input, args.groups, args.output, args.count, args.exposures, args.seed)
    print(json.dumps({'positions': report['positions'], 'manifest': str(args.output / 'manifest.json')}))


if __name__ == '__main__':
    main()

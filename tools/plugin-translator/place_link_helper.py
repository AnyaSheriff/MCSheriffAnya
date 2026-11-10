"""SOURCE PROPOSAL ONLY; importing never builds or writes. Root review before activation."""
import json

def dependency_command():
    # Select one registry dependency package, never root core/test/examples targets.
    return ['cargo','build','--release','--locked','-p','serde_json','--message-format=json']

def choose_artifacts(log):
    artifacts=[]
    for line in log.splitlines():
        try: row=json.loads(line)
        except ValueError: continue
        if row.get('reason')!='compiler-artifact': continue
        if row['target']['name']=='mcsheriffanya': raise ValueError('unexpected core compilation')
        artifacts.append(row)
    matches=[f for row in artifacts if row['target']['name']=='serde_json' for f in row['filenames'] if f.endswith('.rlib')]
    if len(matches)!=1: raise ValueError('serde_json rlib must be unique')
    return matches[0],artifacts

def integration_command(rustc,source,output,rlib,dependency_dir):
    return [rustc,'--edition=2024','--test','--crate-name','world_startup','-C','opt-level=3','-C','debug-assertions=off',str(source),'--extern','serde_json='+str(rlib),'-L','dependency='+str(dependency_dir),'-o',str(output)]

def confined_file(path,root):
    from pathlib import Path
    path=Path(path);root=Path(root).resolve()
    if '..' in path.parts or path!=path.resolve():raise ValueError('noncanonical artifact path')
    if not path.is_absolute() or not path.is_file() or path.is_symlink():raise ValueError('invalid artifact file')
    resolved=path.resolve()
    if not resolved.is_relative_to(root):raise ValueError('artifact outside dependency directory')
    current=path
    while current!=root:
        if current.is_symlink():raise ValueError('symlink artifact path')
        current=current.parent
    return resolved

def validate_command(command):
    if command!=dependency_command():raise ValueError('dependency command changed before spawn')

import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,writeFile,readFile,rm,readdir,readlink,realpath} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {execFileSync,spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'

const installer=resolve('install.sh')
async function fixture(run){
  const root=await mkdtemp(join(tmpdir(),'crabot-install-'))
  try{
    const home=join(root,'home'),assets=join(root,'assets'),bin=join(root,'bin'),bundle=join(root,'bundle/crabot')
    for(const path of [home,assets,bin,join(bundle,'bin'),join(bundle,'libexec'),join(bundle,'web'),join(bundle,'skills/system/management/management-guide'),join(bundle,'lib/lbug/fts')])await mkdir(path,{recursive:true})
    for(const path of ['bin/crabot','libexec/agent-node'])await writeFile(join(bundle,path),'#!/bin/sh\nprintf "installed-fixture\\n"\n',{mode:0o755})
    await writeFile(join(bundle,'bin/crabot'),await readFile(resolve('scripts/release/crabot')),{mode:0o755})
    await writeFile(join(bundle,'libexec/install.sh'),await readFile(installer))
    await writeFile(join(bundle,'web/index.html'),'fixture')
    await writeFile(join(bundle,'skills/system/management/management-guide/SKILL.md'),'fixture')
    await writeFile(join(bundle,'lib/lbug/fts/libfts.lbug_extension'),'fixture-extension')
    const os=execFileSync('uname',['-s'],{encoding:'utf8'}).trim(),arch=execFileSync('uname',['-m'],{encoding:'utf8'}).trim()
    const target=({'Darwin:arm64':'aarch64-apple-darwin','Darwin:x86_64':'x86_64-apple-darwin','Linux:x86_64':'x86_64-unknown-linux-gnu','Linux:aarch64':'aarch64-unknown-linux-gnu','Linux:arm64':'aarch64-unknown-linux-gnu'})[os+':'+arch]
    assert.ok(target,'Unsupported test platform')
    const archive=join(assets,`crabot-${target}.tar.gz`)
    execFileSync('tar',['-czf',archive,'-C',join(root,'bundle'),'crabot'])
    await writeFile(archive+'.sha256',createHash('sha256').update(await readFile(archive)).digest('hex')+'  fixture\n')
    // Offline release transport; still exercises real archive checks, checksum and installation.
    await writeFile(join(bin,'curl'),`#!${process.execPath}
const fs=require('node:fs'),path=require('node:path'),args=process.argv.slice(2);
const url=args.find(v=>v.startsWith('https://')),out=args[args.indexOf('-o')+1];
if(url.endsWith('/latest')){process.stdout.write('https://github.com/wexyx/crabot/releases/tag/v0.0.1');process.exit(0)}
fs.copyFileSync(path.join(process.env.FIXTURE_ASSETS,path.basename(url)),out);
`,{mode:0o755})
    const prefix=join(home,"custom path '$install")
    const env={...process.env,HOME:home,CRABOT_INSTALL_PREFIX:prefix,CRABOT_VERSION:'v0.0.0',FIXTURE_ASSETS:assets,PATH:bin+':/usr/bin:/bin:/usr/sbin:/sbin'}
    delete env.ZDOTDIR;delete env.XDG_CONFIG_HOME
    await run({root,home,prefix,env,archive})
  }finally{await rm(root,{recursive:true,force:true})}
}

test('installer configures Bash command discovery, preserves profiles and is idempotent',async()=>fixture(async({home,prefix,env})=>{
  const profile=join(home,'.bashrc'),original='# user settings\nexport USER_SETTING=preserved\n'
  await writeFile(profile,original)
  for(let i=0;i<2;i++){
    const installed=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/bash'},encoding:'utf8'})
    assert.equal(installed.status,0,installed.stderr)
    const result=spawnSync('/bin/bash',['--noprofile','--norc','-c','. "$HOME/.profile"; . "$HOME/.bashrc"; command -v crabot; crabot; printf "%s" "$USER_SETTING"'],{env,encoding:'utf8'})
    assert.equal(result.status,0,result.stderr)
    assert.ok(result.stdout.includes(prefix+'/bin/crabot'))
    assert.ok(result.stdout.includes('installed-fixture'))
    assert.ok(result.stdout.endsWith('preserved'))
  }
  const content=await readFile(profile,'utf8')
  assert.ok(content.startsWith(original))
  assert.equal(content.split('# Crabot PATH').length-1,1)
  const backups=(await readdir(home)).filter(p=>p.startsWith('.bashrc.crabot-backup.'))
  assert.equal(backups.length,1)
  assert.equal(await readFile(join(home,backups[0]),'utf8'),original)
}))

test('installer writes zsh and fish startup configuration without replacing user settings',async()=>fixture(async({home,env})=>{
  for(const shell of ['zsh','fish']){
    const installed=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/'+shell},encoding:'utf8'})
    assert.equal(installed.status,0,installed.stderr)
    const file=shell==='zsh'?join(home,'.zshrc'):join(home,'.config/fish/conf.d/crabot.fish')
    assert.match(await readFile(file,'utf8'),/# Crabot PATH/)
    if(shell==='zsh' && process.platform==='darwin'){
      const result=spawnSync('/bin/zsh',['-f','-c','source "$HOME/.zshrc"; crabot'],{env,encoding:'utf8'})
      assert.equal(result.status,0,result.stderr)
      assert.match(result.stdout,/installed-fixture/)
    }
  }
}))

test('checksum failure does not create a command or change shell configuration',async()=>fixture(async({home,env,archive})=>{
  await writeFile(archive+'.sha256','0'.repeat(64)+'  fixture\n')
  const result=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/bash'},encoding:'utf8'})
  assert.notEqual(result.status,0)
  assert.match(result.stderr,/Checksum mismatch/)
  assert.doesNotMatch(result.stderr,/READY|\u001b/)
  assert.deepEqual(await readdir(home),[])
}))

test('installer has readable stages without escape codes when piped or NO_COLOR is set',async()=>fixture(async({env})=>{
 for(const extra of [{TERM:'xterm-256color'},{TERM:'dumb',NO_COLOR:'1'}]){
  const result=spawnSync('bash',[installer],{env:{...env,...extra},encoding:'utf8'})
  assert.equal(result.status,0,result.stderr)
  assert.match(result.stderr,/CRABOT \/ INSTALLER/)
  for(let stage=1;stage<=5;stage++)assert.ok(result.stderr.includes(`[${stage}/5]`))
  assert.match(result.stderr,/SHA-256 verified/)
  assert.match(result.stderr,/READY.*v0\.0\.0/)
  assert.doesNotMatch(result.stderr,/\u001b|\r/)
 }
}))

test('interactive installer renders color and respects NO_COLOR on a terminal',async()=>fixture(async({env})=>{
 const harness=`import os,pty,sys,select,time,signal
pid,fd=pty.fork()
if pid==0: os.execvp('bash',['bash',sys.argv[1]])
output=bytearray()
try:
 deadline=time.monotonic()+15
 while time.monotonic()<deadline:
  if select.select([fd],[],[],.1)[0]:
   try: chunk=os.read(fd,65536)
   except OSError: break
   if not chunk: break
   output.extend(chunk)
 else: raise RuntimeError('installer timed out')
 _,status=os.waitpid(pid,0);pid=None
 sys.stdout.buffer.write(output)
 sys.exit(os.waitstatus_to_exitcode(status))
finally:
 if pid is not None: os.kill(pid,signal.SIGKILL);os.waitpid(pid,0)
 os.close(fd)
`
 for(const color of [true,false]){
  const environment={...env,TERM:'xterm-256color'}
  if(color)delete environment.NO_COLOR;else environment.NO_COLOR='1'
  const result=spawnSync('python3',['-c',harness,installer],{env:environment,encoding:'utf8',timeout:20000})
  assert.equal(result.status,0,result.stderr)
  assert.match(result.stdout,/READY/)
  if(color){assert.match(result.stdout,/\u001b\[1;36m/);assert.match(result.stdout,/████/)}
  else {assert.doesNotMatch(result.stdout,/\u001b/);assert.match(result.stdout,/CRABOT \/ INSTALLER/)}
 }
}))

test('updater installer preserves the existing custom prefix and data, and rejects broken downloads',async()=>fixture(async({home,prefix,env,archive})=>{
 const installed=spawnSync('bash',[installer],{env,encoding:'utf8'})
 assert.equal(installed.status,0,installed.stderr)
 const data=join(home,'.crabot')
 await mkdir(data)
 await writeFile(join(data,'state.jsonl'),'keep-history')
 const command=join(prefix,'bin/crabot')
 const previous=await readlink(command)
 const updateEnv={...env,CRABOT_VERSION:'latest'}
 const updated=spawnSync('bash',[installer],{env:updateEnv,encoding:'utf8'})
 assert.equal(updated.status,0,updated.stderr)
 const current=await readlink(command)
 assert.notEqual(current,previous)
 assert.ok((await realpath(current)).startsWith((await realpath(prefix))+'/share/crabot/releases/v0.0.1-'),current)
 assert.equal(await readFile(join(data,'state.jsonl'),'utf8'),'keep-history')
 await writeFile(archive+'.sha256','0'.repeat(64)+'  fixture\n')
 const failed=spawnSync('bash',[installer],{env:updateEnv,encoding:'utf8'})
 assert.notEqual(failed.status,0)
 assert.equal(await readlink(command),current)
}))

import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {modelFixture,pause} from './admin-fixture.mjs'

test('CLI renders a Crabot tool roundtrip as conversation and restores its prompt',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-cli-chat-'))
  const model=await modelFixture()
  let child,output=''
  try{
    child=spawn(resolve('target/debug/agent-node'),['--cli','--web-port','0'],{cwd:dir,env:{PATH:process.env.PATH,CRABOT_DATA_DIR:dir,...model.env},stdio:['pipe','pipe','pipe']})
    child.stdout.on('data',data=>output+=data)
    child.stderr.on('data',data=>output+=data)
    const wait=async text=>{
      for(let i=0;i<200;i++){
        if(output.includes(text))return
        if(child.exitCode!==null)throw new Error(output)
        await pause(25)
      }
      throw new Error(`Missing ${text}: ${output}`)
    }
    await wait('group> ');child.stdin.write('/manage\n');await wait('admin> ')
    child.stdin.write('在哪 TEST_PLAN:[{"name":"group_list","input":{}}]\n')
    await wait('Fixture complete\nadmin> ')
    assert.match(output,/调用工具：group_list/)
    assert.match(output,/│ Agent\nFixture complete/)
    assert.equal(output.match(/Fixture complete/g)?.length,1)
    assert.ok(!output.includes('"type"')&&!output.includes('"seq"')&&!output.includes('"status"'),output)
    const exited=once(child,'exit');child.stdin.end('/exit\n');await exited
    assert.equal(child.exitCode,0,output)
  }finally{
    if(child&&child.exitCode===null&&child.signalCode===null){const done=once(child,'exit');child.kill('SIGTERM');await done}
    await model.close();await rm(dir,{recursive:true,force:true})
  }
})

import test from 'node:test'
import assert from 'node:assert/strict'
import {conversationView} from './conversation-view.js'
import {compactProcessSummary} from './process-summary.js'

test('legacy OpenCode execute history recovers code from its result without rewriting history',()=>{
 const input={code:'const r = await tools.crabot_tool_find({ target: "tool", query: "shell" }); return JSON.stringify(r);'}
 const events=[
  {id:'event-one',seq:1,type:'agent.tool.started',agent:'peer',content:JSON.stringify({id:'call',name:'execute'})},
  {id:'event-two',seq:2,type:'agent.tool.finished',agent:'peer',content:JSON.stringify({id:'call',output:JSON.stringify({name:'execute',input,status:'completed',output:'ok'})})},
 ]
 const before=JSON.stringify(events)
 const row=conversationView(events)[0]
 assert.equal(compactProcessSummary(row),'已完成 · execute · find · tool : "shell"')
 assert.equal(JSON.stringify(events),before)
})
test('discussion yielding is a compact notice, not an assistant bubble',()=>{
 const events=[
  {seq:1,type:'agent.progress',agent:'reviewer',invocation_id:'r1'},
  {seq:2,type:'agent.yield',agent:'reviewer',invocation_id:'r1',content:'已让出本轮'},
  {seq:3,type:'agent.message',agent:'writer',invocation_id:'w1',content:'处理结果'},
  {seq:4,type:'agent.message',aggregate:true,content:'reviewer: 本轮让出。'},
 ]
 assert.deepEqual(conversationView(events.slice(0,2)).map(r=>[r.type,r.text]),[['status','reviewer 已让出本轮']])
 assert.deepEqual(conversationView(events).filter(r=>r.type==='assistant').map(r=>r.text),['处理结果'])
 assert.equal(conversationView([{type:'agent.message',content:'本轮让出。'}])[0].type,'assistant')
})
test('accepted yields display the reason in the notice',()=>{
 const rows=conversationView([{type:'agent.yield',agent:'reviewer',content:'已让出本轮 · 仅涉及翻译，没有待审核事项。'}])
 assert.equal(rows[0].type,'status')
 assert.equal(rows[0].text,'reviewer 已让出本轮 · 仅涉及翻译，没有待审核事项。')
})
test('discussion completion and unclaimed notices remain visible without aggregate duplicates',()=>{
 const rows=conversationView([
  {seq:1,type:'agent.activity',content:'本轮没有成员认领当前事项。'},
  {seq:2,type:'agent.message',aggregate:true,content:'hidden aggregate'},
 ])
 assert.equal(rows.length,1)
 assert.equal(rows[0].type,'status')
 assert.equal(rows[0].text,'本轮没有成员认领当前事项。')
})
test('hundreds of stream fragments produce one answer, not hundreds of cards',()=>{
  const events=[{seq:1,type:'user',content:'在哪'}]
  for(let i=0;i<311;i++)events.push({seq:i+2,type:'text_delta',text:'字'})
  events.push({seq:313,type:'completed',text:'字'.repeat(311)})
  const rows=conversationView(events)
  assert.equal(rows.filter(r=>r.type==='assistant').length,1)
  assert.equal(rows[1].text.length,311)
  assert.equal(rows.at(-1).text,'已完成')
  assert.equal(events.length,313)
})
test('tool boundaries, checkpoints, interrupted partial answers and later turns stay distinct',()=>{
  const rows=conversationView([
    {seq:1,type:'text_delta',text:'正在检查'},
    {seq:2,type:'tool_started',name:'group_list'},
    {seq:3,type:'tool_finished',output:'[]'},
    {seq:4,type:'context_checkpoint',content:'internal'},
    {seq:5,type:'text_delta',text:'部分回答'},
    {seq:6,type:'failed',message:'interrupted'},
    {seq:7,type:'user',content:'继续'},
    {seq:8,type:'completed',text:'最终回答'},
  ])
  assert.deepEqual(rows.filter(r=>r.type==='assistant').map(r=>r.text),['正在检查','部分回答','最终回答'])
  assert.ok(!rows.some(r=>r.type==='context_checkpoint'))
  assert.ok(rows.some(r=>r.type==='failed'&&r.text==='interrupted'))
})

test('members stream into separate bubbles and group aggregate never duplicates replies',()=>{
 const rows=conversationView([
 {seq:1,type:'agent.delta',agent:'alice',invocation_id:'a1',content:'A'},
 {seq:2,type:'agent.delta',agent:'bob',invocation_id:'b1',content:'B'},
 {seq:3,type:'agent.delta',agent:'alice',invocation_id:'a1',content:'!'},
 {seq:4,type:'agent.message',agent:'bob',invocation_id:'b1',content:'B!'},
 {seq:5,type:'agent.message',agent:'alice',invocation_id:'a1',content:'A!'},
 {seq:6,type:'agent.message',agent:'alice',invocation_id:'a2',content:'Next round'},
 {seq:7,type:'agent.message',aggregate:true,content:'Full transcript'},
 ])
 assert.deepEqual(rows.filter(r=>r.type==='assistant').map(r=>[r.label,r.text]),[['alice','A!'],['bob','B!'],['alice','Next round']])
})
test('live tool JSON remains a tool event, not assistant text',()=>{
 const rows=conversationView([{seq:1,type:'agent.tool.started',agent:'a',invocation_id:'a1',content:'{"name":"shell","input":{"command":"pwd"}}'},{seq:2,type:'agent.tool.finished',agent:'a',invocation_id:'a1',content:'{"output":"done"}'}])
 assert.equal(rows.length,1);assert.equal(rows[0].type,'tool');assert.equal(rows[0].pending,false)
})
test('deliberation folds into progress and never becomes part of the answer',()=>{
 const events=[{seq:1,type:'user',content:'在哪'}]
 for(let i=0;i<120;i++)events.push({seq:i+2,type:'agent.reasoning',agent:'default',invocation_id:'r1',content:JSON.stringify({type:'reasoning_delta',text:'想'})})
 for(let i=0;i<120;i++)events.push({seq:200+i,type:'agent.reasoning',agent:'default',invocation_id:'r1',content:JSON.stringify({type:'reasoning_delta',text:'法'})})
 events.push({seq:400,type:'agent.delta',agent:'default',invocation_id:'r1',content:'答案'})
 events.push({seq:401,type:'agent.message',agent:'default',invocation_id:'r1',content:'答案'})
 const rows=conversationView(events)
 const answers=rows.filter(r=>r.type==='assistant')
 assert.equal(answers.length,1)
 assert.equal(answers[0].text,'答案')
 const folded=rows.filter(r=>r.name==='reasoning')
 assert.equal(folded.length,1)
 assert.equal(folded[0].text.length,240)
 assert.ok(!rows.some(r=>r.type==='assistant'&&r.text.includes('想')))
})

test('a new turn starts a fresh folded deliberation row instead of merging',()=>{
 const rows=conversationView([
  {seq:1,type:'agent.reasoning',agent:'default',invocation_id:'a',content:JSON.stringify({type:'reasoning_delta',text:'A'})},
  {seq:2,type:'agent.message',agent:'default',invocation_id:'a',content:'first'},
  {seq:3,type:'user',content:'again'},
  {seq:4,type:'agent.reasoning',agent:'default',invocation_id:'b',content:JSON.stringify({type:'reasoning_delta',text:'B'})},
  {seq:5,type:'agent.message',agent:'default',invocation_id:'b',content:'second'},
 ])
 assert.deepEqual(rows.filter(r=>r.name==='reasoning').map(r=>r.text),['A','B'])
 assert.deepEqual(rows.filter(r=>r.type==='assistant').map(r=>r.text),['first','second'])
})

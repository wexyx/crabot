export const modes=[
 {id:'chat',icon:'◇',name:'简单聊天',caption:'一个 Agent，直接对话',description:'有且仅有一个成员，也可选择对外表现为单个 Agent 的虚拟 Agent。'},
 {id:'relay',icon:'↪',name:'接力模式',caption:'按优先级依次接手',description:'手动排序、随机排列或协商接手顺序；Token 不足时切换下一位。'},
 {id:'a2a',icon:'⇄',name:'讨论模式',caption:'按职责讨论，达成共识后结束',description:'成员先判断职责，无关时让出；有结论且其他成员确认或让出后提前结束，轮次仅为上限。'},
 {id:'pmo',icon:'◎',name:'Leader 模式',caption:'Leader 决策与任务分配',description:'Leader 根据成员角色制定计划、分配任务，并汇总执行结果。'},
]
export function validateGroupConfiguration(draft){
 if(new TextEncoder().encode(draft.name.trim()).length>256)return '名称不能超过 256 字节。'
 const p=draft.policy
 if(p.mode==='chat'&&p.members.length!==1)return '简单聊天必须且只能选择一个 Agent。'
 if(!['manual','random','negotiated'].includes(p.relay_strategy||'manual'))return '接力策略无效。'
 if(!modes.some(m=>m.id===p.mode))return '请选择有效的交流模式。'
 if(p.members.length<1||p.members.length>8)return '群成员需要 1–8 个 Agent。'
 if(!Number.isInteger(p.rounds)||p.rounds<1||p.rounds>60)return '交流轮次需要是 1–60 的整数。'
 if(new TextEncoder().encode(p.instructions).length>8192)return '协作要求不能超过 8192 字节。'
 for(let i=0;i<p.members.length;i++){
  const member=p.members[i]
  if(!member.path.length||member.path.some(s=>!s.trim()))return 'Agent 路径不能为空。'
  if(new TextEncoder().encode(member.role).length>255)return '成员角色不能超过 255 字节。'
  if(p.members.slice(0,i).some(m=>m.path.every((s,j)=>s===member.path[j])||member.path.every((s,j)=>s===m.path[j])))return '成员路径重复，或包含重叠的子树。'
 }
 if(p.mode==='pmo'&&!p.members.some(m=>JSON.stringify(m.path)===JSON.stringify(p.leader)))return 'Leader 模式需要从群成员中选择 Leader。'
 return ''
}

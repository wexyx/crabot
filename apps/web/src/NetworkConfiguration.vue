<script setup>
import {ref,onMounted,onBeforeUnmount} from 'vue'
import ConfigPanel from './ConfigPanel.vue'
import PeerConnections from './PeerConnections.vue'
const props=defineProps({project:String,request:Function})
defineEmits(['close'])
const mounts=ref([]),pending=ref(null),error=ref(''),busy=ref(false)
let alive=true,timer,loading=false
async function load(){if(loading)return;loading=true;try{const [result,approvals]=await Promise.all([props.request('/v1/repl/'+props.project+'/peers'),props.request('/v1/admin-agent/'+props.project+'/approvals')]);if(alive){mounts.value=result;pending.value=approvals.find(a=>a.tool==='peer_mount')||null}}catch(e){if(alive)error.value=e.message}finally{loading=false}}
async function decide(allow){busy.value=true;error.value='';try{await props.request('/v1/admin-agent/'+props.project+'/approvals/'+pending.value.id,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({allow})});pending.value=null;await load()}catch(e){error.value=e.message}finally{busy.value=false}}
onMounted(()=>{load();timer=setInterval(()=>{if(!document.hidden)load()},5000)})
onBeforeUnmount(()=>{alive=false;clearInterval(timer)})
</script>
<template>
 <ConfigPanel title="组网" eyebrow="CONNECTIONS" description="管理当前 Crabot 与其它节点的连接，独立于 Agent 配置。" inline :busy="busy" @close="$emit('close')">
  <div class="config-detail network-config"><p v-if="error" class="config-alert danger" role="alert">{{error}}</p>
   <PeerConnections :request="request" :project="project" :mounts="mounts" :busy="busy||!!pending" @pending="pending=$event" @changed="load"/>
   <el-alert type="warning" :closable="false" show-icon v-if="pending" class="approval"><h3>确认连接授权</h3><p>{{pending.warning}}</p><p>{{pending.input?.url}}</p><el-button type="primary" native-type="button" :disabled="busy" @click="decide(true)">确认连接</el-button><el-button type="default" native-type="button" class="secondary" :disabled="busy" @click="decide(false)">拒绝</el-button></el-alert>
  </div>
 </ConfigPanel>
</template>
<style scoped>.network-config{display:flex;flex-direction:column;gap:24px;overflow:auto}.network-config>*{flex-shrink:0}</style>

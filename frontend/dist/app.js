const $ = (s) => document.querySelector(s);
const native = () => window.go?.main?.App;
let lastStatus = null;

const pages = {
  home:["Home","Device overview and connection health"],
  lighting:["Lighting","Verified onboard effects for the active keyboard"],
  diagnostics:["Diagnostics","Connection, command and recovery log"]
};

function showPage(name){
  document.querySelectorAll('.page').forEach(p=>p.classList.remove('active'));
  document.querySelectorAll('#nav button').forEach(b=>b.classList.toggle('active',b.dataset.page===name));
  document.querySelector(`#page-${name}`)?.classList.add('active');
  $('#pageTitle').textContent=pages[name]?.[0]||name;
  $('#pageSubtitle').textContent=pages[name]?.[1]||'';
  if(name==='diagnostics') refreshLogs();
}

document.querySelectorAll('#nav button:not(:disabled)').forEach(b=>b.addEventListener('click',()=>showPage(b.dataset.page)));
$('#brightness').addEventListener('input',e=>$('#brightnessOut').value=e.target.value);
$('#speed').addEventListener('input',e=>$('#speedOut').value=e.target.value);

async function loadModes(){
  const fallback=["Off","Static","Single On","Single Off","Glittering","Falling","Colourful","Breath","Spectrum","Outward","Scrolling","Rolling","Rotating","Explode","Launch","Ripples","Flowing","Pulsating","Tilt","Shuttle"];
  let modes=fallback;
  try{if(native()) modes=await native().GetLightingModes();}catch(e){}
  $('#lightingMode').innerHTML=modes.map((m,i)=>`<option value="${i}">${i}. ${m}</option>`).join('');
}

function stateClass(s){return ['ready','busy','recovery','error','unsupported'].includes(s)?s:'disconnected'}
function updateStatus(s){
  lastStatus=s;
  const state=s?.state||'disconnected', info=s?.info||{};
  const pill=$('#connectionPill'); pill.textContent=state; pill.className=`pill ${stateClass(state)}`;
  $('#deviceName').textContent=info.name||'AULA F108 Pro';
  $('#deviceDetail').textContent=s?.lastError || (state==='ready'?'Configuration interface ready':'Waiting for a supported keyboard...');
  $('#connectionType').textContent=info.connection||'—';
  $('#vidpid').textContent=info.vendorId?`${info.vendorId.toString(16).padStart(4,'0').toUpperCase()} / ${info.productId.toString(16).padStart(4,'0').toUpperCase()}`:'—';
  $('#stateText').textContent=state.charAt(0).toUpperCase()+state.slice(1);
  $('#applyLighting').disabled=state!=='ready';
}

async function pollStatus(){
  try{
    if(native()) updateStatus(await native().GetStatus());
    else updateStatus({state:'unsupported',lastError:'Browser preview. Hardware access is available inside the Windows desktop build.',info:{name:'AULA F108 Pro'}});
  }catch(e){ updateStatus({state:'error',lastError:String(e),info:{}}); }
}

function hexRGB(hex){const n=parseInt(hex.slice(1),16);return [(n>>16)&255,(n>>8)&255,n&255]}
$('#applyLighting').addEventListener('click',async()=>{
  const [r,g,b]=hexRGB($('#lightingColor').value);
  const cfg={mode:Number($('#lightingMode').value),r,g,b,brightness:Number($('#brightness').value),speed:Number($('#speed').value),direction:Number($('#direction').value),colorful:$('#colorful').checked};
  $('#lightingResult').textContent='Applying...'; $('#applyLighting').disabled=true;
  try{
    if(!native()) throw new Error('Hardware calls are available only in the desktop build.');
    await native().ApplyLighting(cfg); $('#lightingResult').textContent='Applied successfully.';
  }catch(e){ $('#lightingResult').textContent=`Failed: ${e}`; }
  await pollStatus();
});

$('#reconnectButton').addEventListener('click',async()=>{try{await native()?.Reconnect();}catch(e){} setTimeout(pollStatus,250)});
async function refreshLogs(){
  try{
    const logs=native()?await native().GetLogs():[];
    $('#logView').textContent=logs.length?logs.map(x=>`${new Date(x.time).toLocaleTimeString()} [${x.level.toUpperCase()}] ${x.message}`).join('\n'):'No log entries yet.';
  }catch(e){$('#logView').textContent=String(e)}
}
$('#refreshLogs').addEventListener('click',refreshLogs);

(async function boot(){
  await loadModes(); await pollStatus();
  setTimeout(()=>{$('#splash').classList.add('hidden');$('#app').classList.remove('hidden')},850);
  setInterval(pollStatus,1200);
})();

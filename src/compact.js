// Glass House Compact — strip, compress, rename (global mapping), bundle
var fs=new ActiveXObject("Scripting.FileSystemObject");
function trim(s){return s.replace(/^\s+|\s+$/g,'');}
var RES={};RES['break']=1;RES['case']=1;RES['catch']=1;RES['continue']=1;RES['delete']=1;RES['do']=1;RES['else']=1;RES['finally']=1;RES['for']=1;RES['function']=1;RES['if']=1;RES['in']=1;RES['instanceof']=1;RES['new']=1;RES['return']=1;RES['switch']=1;RES['this']=1;RES['throw']=1;RES['try']=1;RES['typeof']=1;RES['var']=1;RES['void']=1;RES['while']=1;RES['with']=1;RES['class']=1;RES['const']=1;RES['let']=1;RES['true']=1;RES['false']=1;RES['null']=1;RES['undefined']=1;RES['NaN']=1;RES['Infinity']=1;RES['arguments']=1;RES['eval']=1;RES['constructor']=1;RES['prototype']=1;RES['window']=1;RES['document']=1;RES['console']=1;RES['Math']=1;RES['JSON']=1;RES['Date']=1;RES['Array']=1;RES['Object']=1;RES['String']=1;RES['Number']=1;RES['Boolean']=1;RES['RegExp']=1;RES['Error']=1;RES['Promise']=1;RES['Map']=1;RES['Set']=1;RES['WeakMap']=1;RES['WeakSet']=1;RES['Symbol']=1;RES['parseInt']=1;RES['parseFloat']=1;RES['isNaN']=1;RES['isFinite']=1;RES['setTimeout']=1;RES['clearTimeout']=1;RES['setInterval']=1;RES['clearInterval']=1;RES['performance']=1;RES['localStorage']=1;RES['sessionStorage']=1;RES['XMLHttpRequest']=1;RES['Blob']=1;RES['URL']=1;RES['TextEncoder']=1;RES['TextDecoder']=1;RES['Uint8Array']=1;RES['Int32Array']=1;RES['location']=1;RES['history']=1;RES['navigator']=1;RES['fetch']=1;RES['GlassHouse']=1;
function isReserved(n){return RES[n]||false;}

function renameIdents(source,gp,gim,grm){
    var strs=[],re=/'[^'\\]*(?:\\.[^'\\]*)*'|"[^"\\]*(?:\\.[^"\\]*)*"/g;
    source=source.replace(re,function(m){strs.push(m);return '\x10'+(strs.length-1)+'\x11';});
    var freq={},ordered=[],idre=/\b[a-zA-Z_$][a-zA-Z0-9_$]*\b/g,m;
    while((m=idre.exec(source))!==null){var id=m[0];if(!isReserved(id)&&id.length>1&&!gim[id]){if(!freq[id]){freq[id]=0;ordered.push(id);}freq[id]++;}}
    ordered.sort(function(a,b){return freq[b]-freq[a]||a.length-b.length;});
    var nx=gp.nextIdx||0,cc='abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_';
    for(var i=0;i<ordered.length;i++){
        var idx=nx%cc.length,code=cc.charAt(idx);if(nx>=cc.length)code=cc.charAt(idx)+String(Math.floor(nx/cc.length));
        while(isReserved(code)||grm[code]){nx++;idx=nx%cc.length;code=cc.charAt(idx);if(nx>=cc.length)code=cc.charAt(idx)+String(Math.floor(nx/cc.length));}
        gim[ordered[i]]=code;grm[code]=ordered[i];nx++;
    }
    gp.nextIdx=nx;
    source=source.replace(idre,function(id){return gim[id]||id;});
    source=source.replace(/\x10(\d+)\x11/g,function(_,idx){return strs[parseInt(idx)]||'';});
    return{source:source,renamed:ordered.length};
}

try{
    var inp=WScript.Arguments(0),outp=WScript.Arguments(1);
    var lf=fs.OpenTextFile(inp,1),paths=lf.ReadAll().split('\n');lf.Close();

    var totalOrig=0,bundle='/* Glass House */\n(function(){\n"use strict";\n';
    var gp={nextIdx:0},gim={},grm={};

    for(var i=0;i<paths.length;i++){
        var p=trim(paths[i]);if(!p)continue;
        var s='';try{var f=fs.OpenTextFile(p,1);s=f.ReadAll();f.Close();}catch(e){continue;}
        totalOrig+=s.length;
        s=s.replace(/\/\/.*$/gm,'');
        s=s.replace(/\/\*[\s\S]*?\*\//g,'');
        var out='',inStr=false,sc='',lastSpace=false;
        for(var j=0,sl=s.length;j<sl;j++){var c=s.charAt(j);
            if((c==='"'||c==="'")&&!inStr){inStr=true;sc=c;out+=c;continue;}
            if(inStr&&c===sc){inStr=false;out+=c;continue;}
            if(inStr){out+=c;continue;}
            if(c===' '||c==='\t'||c==='\n'||c==='\r'){if(!lastSpace){out+=' ';lastSpace=true;}continue;}
            lastSpace=false;out+=c;
        }
        s=trim(out);if(!s)continue;
        var r=renameIdents(s,gp,gim,grm);s=r.source;
        bundle+='\n/* '+p.replace(/^.*[\\\/]/,'')+' */\n'+s+'\n';
    }
    bundle+='\n})();\n';
    var reduction=totalOrig>0?((1-(bundle.length/totalOrig))*100).toFixed(1):0;
    var of=fs.CreateTextFile(outp,true);of.Write(bundle);of.Close();
    WScript.Echo(reduction+'% reduction | '+(bundle.length/1024).toFixed(1)+' KB');
}catch(e){WScript.Echo('ERROR: '+e.message);WScript.Quit(1);}

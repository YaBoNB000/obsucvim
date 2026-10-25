function(b,u,z,Z,Z,Z,Z)local Z,o,w,K,G,q,M,F,H,E,V,f,B,j,d,Y,_,g,l,k,m,x,R,t,J,S,O,y,A,a,X,N=b[22],b[34],b[102],b[15],b[13],b[53],b[99],b[20],b[10],b[8],b[71],b.AL,b[25],b[114],b[66],b[47],b[122],b[31],b.UL,0
while true do 
if 
k<=0
 m=u[15]
x=u[9]
R=u[11]
t=u[13]
J=u[16]
S=u[7]
O=u[5]
y=u[14]
A=u[10]
a=u[12]
X=u[8]
k,N=1,function(...)local k,s,W,p,L,C,I,h
local n=Z(a)
local a,r,e
local Q,T=S
local S,c=O,o()
local o
if S==251
 while 
true do 
local 
O=J[Q]
if O>=19
 if 
O<29
 if 
O>=24
 if 
O<26
 local 
i=Q
if O~=25
 S,Q=m[i],R[i]+1
break
else p,I={...},y[i]
w(p,1,m[i],I,n)
end
elseif O>=27
 if 
O~=28
 p=z[m[Q]]
p[4][p[7]]=n[R[Q]]
else Q=if n[y[Q]]then R[Q]else m[Q]
end
else if 
o
 for 
i in K,o,nil do 
if 
o
 local 
D=o[i]
if D
 D[4]=D
D[6]=n[i]
D[7]=6
o[i]=nil
end
end
end
end
return
end
elseif O>=21
 if 
O<22
 p,I,T=R[Q],{...},m[Q]
w(I,1,p-1,T,n)
n[T+p-1]=G(q(p,...))
elseif O~=23
 n[y[Q]]=n[R[Q]]
else n[y[Q]](n[m[Q]])
end
elseif O~=20
 n[R[Q]]=n[y[Q]](A[Q])
else if 
o
 for 
q in K,o,nil do 
if 
o
 local 
i=o[q]
if i
 i[4]=i
i[6]=n[q]
i[7]=6
o[q]=nil
end
end
end
end
return n[y[Q]]
end
elseif O<34
 if 
O<31
 if 
O~=30
 n[y[Q]]=n[R[Q]]%m[Q]
else p,I,T=m[Q],R[Q],y[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local q,i=R,b:vL(I)
q[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,15)+b:iL(2147483647,(V((E(i,15)))))))
local D,U,v=y,b:vL(T),b:vL(s)
D[Q]=b:vL(E(U,76)+b:iL(1850771837,4294967295)+(b:iL(2444195459,v)+b:iL(2444195459,(V(v)))))
v,i=m,b:vL(s)
v[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,114)+b:iL(2147483647,(V((E(i,114)))))))
q,i,v,U=J,b:vL(k),b:vL(Q),b:vL(p)
q[Q]=b:vL(E(i,113)+b:iL(2147483648,v)+(b:iL(2147483648,U)+b:iL(2147483648,(E(v,U)))))
Q-=1
end
elseif O>=32
 if 
O~=33
 Q=if n[y[Q]]<=R[Q]then m[Q]else Q
else n[R[Q]]()
end
else p=z[R[Q]]
p[4][p[7]][n[y[Q]]]=n[m[Q]]
end
elseif O>=36
 if 
O>=37
 if 
O~=38
 p,I=y[Q],R[Q]
T={[f]=I-p+1}
w(n,p,I,1,T)
n[m[Q]]=T
else n[m[Q]]=n[y[Q]]==R[Q]
end
else Q=m[Q]
end
elseif O==35
 p=m[Q]
I,T,e=n[p],n[p+1],n[p+2]
n[p]=I(T,e)
else n[m[Q]]=y[Q]*n[R[Q]]
end
elseif O<9
 if 
O>=4
 if 
O>=6
 if 
O>=7
 if 
O~=8
 n[R[Q]]=n[m[Q]]+y[Q]
else p=R[Q]+1
for q=1,m[Q],1 do 
I=F(E(y[Q],q),127)
R[p]=E(R[p],I)
y[p]=E(y[p],I)
m[p]=E(m[p],I)
J[p]=E(J[p],I)
p+=1
end
J[Q]=12
end
else n[R[Q]]=t[Q]
end
elseif O==5
 n[y[Q]]=n[R[Q]](n[m[Q]])
else p=t[Q]
local q,i=x[Q],z
local D=q and#q/2 or 0
local U,v=D>0 and{}
if U
 v=o
for P=1,D,1 do 
local 
D=(P-1)*2
local bv,uv=q[D+2],q[D+1]
if bv==2
 v=if not v
{}else v
local q,D=v[uv]
if not q
 q={[7]=uv,[4]=n}
v[uv]=q
D=q
else D=q
end
U[P]=D
elseif bv==1
 U[P]=n[uv]
elseif bv==0
 U[P]={[7]=uv,[4]=n}
elseif bv==3
 U[P]=i[uv]
end
end
else v=o
end
T=b[p[4]](b,p,U,nil)
B(T,c)
n[m[Q]]=T
I,o=U,v
end
elseif O>=2
 if 
O~=3
 Q=n[m[Q]]
else p,I,T=y[Q],R[Q],m[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local q,i,D=R,b:vL(I),b:vL(p)
q[Q]=b:vL(E(i,76)+b:iL(348197941,4294967295)+(b:iL(3946769355,D)+b:iL(3946769355,(V(D)))))
D,q=y,b:vL(s)
D[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,q)+(b:iL(2147483648,67)+b:iL(2147483647,(V((E(q,67)))))))
q,D=m,b:vL(T)
q[Q]=b:vL(b:iL(3746249854,D)+b:iL(3746249854,91)+(b:iL(1097434884,(F(D,91)))+b:iL(548717443,(E(D,91)))))
q,D=J,b:vL(k)
q[Q]=b:vL(b:iL(762416654,D)+b:iL(762416654,112)+(b:iL(2770133988,(F(D,112)))+b:iL(3532550643,(E(D,112)))))
Q-=1
end
elseif O==1
 n[R[Q]]=b[y[Q]]
else p=z[m[Q]]
p[4][p[7]][n[y[Q]]]=x[Q]
end
elseif O<14
 if 
O>=11
 if 
O<12
 p=z[m[Q]]
n[R[Q]]=p[4][p[7]][n[y[Q]]]
elseif O==13
 p=n[R[Q]]
n[m[Q]]=G(j(p,y[Q],p.n))
end
elseif O==10
 n[R[Q]]=z[y[Q]]
else p,I,T=y[Q],m[Q],R[Q]
e=T<16384 and 7 or(T<2097152 and 14 or 21)
s,k=F(T,M(1,e)-1),H(T,e)
local q,i,D=R,b:vL(s),b:vL(p)
q[Q]=b:vL(E(i,87)+b:iL(497547051,4294967295)+(b:iL(3797420245,D)+b:iL(3797420245,(V(D)))))
D,i,q=y,b:vL(p),b:vL(s)
D[Q]=b:vL(E(i,118)+b:iL(905102271,4294967295)+(b:iL(3389865025,q)+b:iL(3389865025,(V(q)))))
i,D=m,b:vL(I)
i[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,D)+(b:iL(2147483648,31)+b:iL(2147483647,(V((E(D,31)))))))
D,q,i=J,b:vL(k),b:vL(Q)
D[Q]=b:vL(E(q,94)+b:iL(131571421,4294967295)+(b:iL(4163395875,i)+b:iL(4163395875,(V(i)))))
Q-=1
end
elseif O<16
 if 
O~=15
 n[y[Q]]=R[Q]
else n[m[Q]]=n[y[Q]]==n[R[Q]]
end
elseif O<17
 local 
q,i,D=u,R[Q],y[Q]
local U=q[2]
q=U[4]
local v=E(q[i],287072149)
q[i]=v
q,i=U[7],v+1
U=d(q,i)
local P,bv
if U<128
 P,bv=U,i+1
else v=d(q,i+1)
if v<128
 P,bv=U-128+v*128,i+2
else local 
uv=d(q,i+2)
if uv<128
 P,bv=U-128+(v-128)*128+uv*16384,i+3
else local 
zv=d(q,i+3)
P,bv=U-128+(v-128)*128+(uv-128)*16384+zv*2097152,i+4
end
end
end
for i=bv,bv+P-1,1 do 
Y(q,i,(E(d(q,i),D)))
end
R[Q],y[Q],m[Q],J[Q]=197,77,228,12
elseif O==18
 p=z[m[Q]]
n[R[Q]]=p[4][p[7]]
else n[m[Q]]=not n[R[Q]]
end
Q+=1
end
end
if S==99
 while 
true do 
local 
q=J[Q]
if q<39
 if 
q<19
 if 
q>=9
 if 
q<14
 if 
q>=11
 if 
q>=12
 if 
q==13
 Q=n[y[Q]]
else n[y[Q]]=m[Q]-n[R[Q]]
end
else n[y[Q]]=n[m[Q]]+R[Q]
end
elseif q==10
 p,I,T=m[Q],R[Q],y[Q]
e=p+I
n[p]=G(n[p](j(n,p+1,e)))
else n[m[Q]]=n[R[Q]]>=n[y[Q]]
end
elseif q<16
 if 
q==15
 for 
O=y[Q],R[Q],1 do 
n[O]=nil
end
else if 
o
 for 
O in K,o,nil do 
if 
o
 local 
i=o[O]
if i
 i[4]=i
i[6]=n[O]
i[7]=6
o[O]=nil
end
end
end
end
return A[Q]
end
elseif q>=17
 if 
q~=18
 n[R[Q]]=b[m[Q]]
else p=m[Q]+1
for O=1,y[Q],1 do 
I=F(E(R[Q],O),127)
R[p]=E(R[p],I)
y[p]=E(y[p],I)
m[p]=E(m[p],I)
J[p]=E(J[p],I)
p+=1
end
J[Q]=58
end
else p,I,T=R[Q],m[Q],y[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local O,i,D=R,b:vL(s),b:vL(k)
O[Q]=b:vL(E(i,111)+b:iL(2704394164,4294967295)+(b:iL(1590573132,D)+b:iL(1590573132,(V(D)))))
D,O=y,b:vL(T)
D[Q]=b:vL(b:iL(149445558,O)+98+(b:iL(4294967294,(F(98,O)))+(b:iL(4145521739,4294967295)+b:iL(149445557,(V(O))))))
i,D,O=m,b:vL(I),b:vL(Q)
i[Q]=b:vL(E(D,81)+b:iL(257896994,4294967295)+(b:iL(4037070302,O)+b:iL(4037070302,(V(O)))))
D,O=J,b:vL(k)
D[Q]=b:vL(b:iL(909667300,O)+b:iL(909667300,84)+(b:iL(2475632696,(_(O,84)))+b:iL(909667301,(E(O,84)))))
Q-=1
end
elseif q<4
 if 
q>=2
 if 
q==3
 p,I,T=m[Q],y[Q],R[Q]
e,s,k=p+T-1,p+I,G(n[p](j(n,p+1,p+I)))
w(k,1,T,p,n)
else n[y[Q]]={}
end
elseif q~=1
 n[m[Q]]=y[Q]
else n[m[Q]]=n[y[Q]]*n[R[Q]]
end
elseif q<6
 if 
q==5
 Q=y[Q]
else n[m[Q]]=t[Q]
end
elseif q<7
 n[m[Q]]=n[R[Q]]<y[Q]
elseif q==8
 n[y[Q]]=n[m[Q]]-n[R[Q]]
else p,I,T=R[Q],y[Q],m[Q]
e=T<16384 and 7 or(T<2097152 and 14 or 21)
s,k=F(T,M(1,e)-1),H(T,e)
local O,i=R,b:vL(p)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,0)+b:iL(2147483647,(V((E(i,0)))))))
O,i=y,b:vL(I)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,90)+b:iL(2147483647,(V((E(i,90)))))))
O,i=m,b:vL(s)
O[Q]=b:vL(b:iL(342375939,i)+b:iL(342375939,89)+(b:iL(3610215418,(_(i,89)))+b:iL(342375940,(E(i,89)))))
O,i=J,b:vL(k)
O[Q]=b:vL(b:iL(1573653233,i)+b:iL(1573653233,104)+(b:iL(1147660830,(F(i,104)))+b:iL(2721314064,(E(i,104)))))
Q-=1
end
elseif q>=29
 if 
q>=34
 if 
q<36
 if 
q~=35
 n[R[Q]]=n[y[Q]]-m[Q]
else n[R[Q]][n[y[Q]]]=n[m[Q]]
end
elseif q>=37
 if 
q==38
 p,I,T=y[Q],R[Q],m[Q]
e=n[p]
w(n,p+1,p+I,T+1,e)
else n[y[Q]]=n[R[Q]]<=m[Q]
end
else n[y[Q]](n[m[Q]])
end
elseif q<31
 if 
q==30
 r,p,I={[8]=C,[6]=r,[5]=W,[9]=L},m[Q],g(l)
I(b,n[p],n[p+1],n[p+2])
Q,L=R[Q],I
else p=z[m[Q]]
p[4][p[7]][n[R[Q]]]=n[y[Q]]
end
elseif q>=32
 if 
q~=33
 n[m[Q]]=y[Q]
n[m[Q+1]]=y[Q+1]
Q+=1
else n[y[Q]]=n[m[Q]](x[Q])
end
else p,I,T=m[Q],R[Q],y[Q]
e,s=n[p],p+I
k=n[s]
w(n,p+1,s-1,T+1,e)
w(k,1,k[f],T+I,e)
end
elseif q<24
 if 
q<21
 if 
q~=20
 p=z[R[Q]]
p[4][p[7]]=t[Q]
else n[y[Q]][m[Q]]=n[R[Q]]
end
elseif q>=22
 if 
q~=23
 n[R[Q]]=not n[m[Q]]
else p,I=m[Q],n[R[Q]]
n[p+1]=I
n[p]=I[t[Q]]
end
else n[m[Q]](n[R[Q]],n[y[Q]])
end
elseif q>=26
 if 
q>=27
 if 
q==28
 n[y[Q]]=#n[R[Q]]
else n[y[Q]]=R[Q]*n[m[Q]]
end
else n[y[Q]]=n[m[Q]]()
end
elseif q~=25
 p=t[Q]
local O,i=A[Q],z
local D=O and#O/2 or 0
local U,v=D>0 and{}
if U
 v=o
for P=1,D,1 do 
local 
D=(P-1)*2
local bv,uv=O[D+2],O[D+1]
if bv==2
 v=if not v
{}else v
local O,D=v[uv]
if not O
 O={[7]=uv,[4]=n}
v[uv]=O
D=O
else D=O
end
U[P]=D
elseif bv==1
 U[P]=n[uv]
elseif bv==0
 U[P]={[7]=uv,[4]=n}
elseif bv==3
 U[P]=i[uv]
end
end
else v=o
end
T=b[p[4]](b,p,U,nil)
B(T,c)
n[R[Q]]=T
I,o=U,v
else local 
O,i,D=u,m[Q],R[Q]
local U=O[2]
O=U[4]
local v=E(O[i],287072149)
O[i]=v
O,i=U[7],v+1
v=d(O,i)
local P,bv
if v<128
 P,bv=v,i+1
else U=d(O,i+1)
if U<128
 P,bv=v-128+U*128,i+2
else local 
uv=d(O,i+2)
if uv<128
 P,bv=v-128+(U-128)*128+uv*16384,i+3
else local 
zv=d(O,i+3)
P,bv=v-128+(U-128)*128+(uv-128)*16384+zv*2097152,i+4
end
end
end
for i=bv,bv+P-1,1 do 
Y(O,i,(E(d(O,i),D)))
end
m[Q],R[Q],y[Q],J[Q]=100,1,242,58
end
elseif q>=58
 if 
q<68
 if 
q<63
 if 
q>=60
 if 
q>=61
 if 
q~=62
 p=z[m[Q]]
p[4][p[7]][t[Q]]=n[R[Q]]
else p,I,T=m[Q],y[Q],R[Q]
e=I<2097152 and 7 or 14
s,k=F(I,M(1,e)-1),H(I,e)
R[Q]=b:vL(E(b:vL(T),67)+b:iL(2840691636,4294967295)+(b:iL(1454275660,67)+b:iL(1454275660,(V(67)))))
local O,i=y,b:vL(s)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,31)+b:iL(2147483647,(V((E(i,31)))))))
local O,i,D=m,b:vL(p),b:vL(Q)
O[Q]=b:vL(E(i,121)+b:iL(1087777524,4294967295)+(b:iL(3207189772,D)+b:iL(3207189772,(V(D)))))
D,O=J,b:vL(k)
D[Q]=b:vL(E(O,100)+b:iL(810990060,4294967295)+(b:iL(3483977236,O)+b:iL(3483977236,(V(O)))))
Q-=1
end
else n[y[Q]]=n[R[Q]]==m[Q]
end
elseif not(q~=59)then p,I,T,e=m[Q],L()
if I
 n[p+1]=T
n[p+2]=e
Q=y[Q]
end
end
elseif q<65
 if 
q==64
 n[m[Q]]=n[R[Q]][y[Q]]
else n[y[Q]]=n[R[Q]]+n[m[Q]]
end
elseif q<66
 n[y[Q]]=n[R[Q]]/m[Q]
elseif q~=67
 p,I,T=y[Q],R[Q],m[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local O,i=R,b:vL(I)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,107)+b:iL(2147483647,(V((E(i,107)))))))
O,i=y,b:vL(s)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,53)+b:iL(2147483647,(V((E(i,53)))))))
local O,i,D=m,b:vL(T),b:vL(Q)
O[Q]=b:vL(E(i,25)+b:iL(488688261,4294967295)+(b:iL(3806279035,D)+b:iL(3806279035,(V(D)))))
i,O=J,b:vL(k)
i[Q]=b:vL(b:iL(4040609347,4294967295)+b:iL(2147483648,O)+(b:iL(2147483648,101)+(b:iL(254357949,(V((E(O,101)))))+b:iL(2401841598,(E(O,101))))))
Q-=1
else n[y[Q]]=n[R[Q]]%A[Q]
end
elseif q<73
 if 
q<70
 if 
q~=69
 n[m[Q]][t[Q]]=n[R[Q]]
else L,C,W,r=r[9],r[8],r[5],r[6]
end
elseif q>=71
 if 
q~=72
 Q=if n[R[Q]]then m[Q]else y[Q]
else p,I,T=m[Q],R[Q],y[Q]
e,s=p+T-1,p+I
k=n[s]
a=k.n
h=I+a-1
k[f]=h
w(k,1,a,I,k)
w(n,p+1,s-1,1,k)
I=G(n[p](j(k,1,k[f])))
w(I,1,T,p,n)
end
else n[y[Q]]=n[R[Q]][A[Q]]
end
elseif q<75
 if 
q==74
 n[y[Q]]=u
else n[y[Q]]=n[R[Q]][n[m[Q]]]
end
elseif q>=76
 if 
q==77
 n[R[Q]]=n[m[Q]]*y[Q]
else p=z[m[Q]]
n[y[Q]]=p[4][p[7]][n[R[Q]]]
end
else n[R[Q]]=E(n[y[Q]],A[Q])
end
elseif q<48
 if 
q>=43
 if 
q>=45
 local 
O=Q
if q>=46
 if 
q~=47
 S,Q=y[O],m[O]+1
break
else n[y[O]]=n[m[O]](n[R[O]])
end
else p=y[O]
I,T,e=n[p],n[p+1],n[p+2]
n[p]=I(T,e)
end
elseif q==44
 n[m[Q]]=Z(y[Q])
else p,I,T=y[Q],m[Q],R[Q]
e=T<2097152 and 7 or 14
s,k=F(T,M(1,e)-1),H(T,e)
local O,i,D=R,b:vL(s),b:vL(Q)
O[Q]=b:vL(E(i,127)+b:iL(2147483648,i)+(b:iL(2147483648,D)+b:iL(2147483648,(E(i,D)))))
D,O=y,b:vL(p)
D[Q]=b:vL(b:iL(3634557271,O)+b:iL(3634557271,18)+(b:iL(1320820050,(_(O,18)))+(b:iL(3634557272,4294967295)+b:iL(660410024,(V((E(O,18))))))))
i,D,O=m,b:vL(I),b:vL(T)
i[Q]=b:vL(E(D,98)+b:iL(44094623,4294967295)+(b:iL(4250872673,O)+b:iL(4250872673,(V(O)))))
D,i,O=J,b:vL(k),b:vL(s)
D[Q]=b:vL(E(i,101)+b:iL(45285643,4294967295)+(b:iL(4249681653,O)+b:iL(4249681653,(V(O)))))
Q-=1
end
elseif q<41
 if 
q~=40
 p=z[R[Q]]
p[4][p[7]]=n[y[Q]]
else n[m[Q]]=t[Q]+x[Q]
end
elseif q~=42
 local 
O=m[Q]
if o
 local 
i=o[O]
if i
 i[4]=i
i[6]=n[O]
i[7]=6
o[O]=nil
end
end
else n[m[Q]]=n[R[Q]]%y[Q]
end
elseif q<53
 if 
q>=50
 if 
q>=51
 if 
q==52
 n[R[Q]](n[m[Q]],t[Q])
else n[y[Q]]=n[m[Q]]==n[R[Q]]
end
else if 
o
 for 
O in K,o,nil do 
if 
o
 local 
i=o[O]
if i
 i[4]=i
i[6]=n[O]
i[7]=6
o[O]=nil
end
end
end
end
return
end
elseif q==49
 p=z[m[Q]]
n[y[Q]]=p[4][p[7]]
else p,I,T=R[Q],m[Q],y[Q]
e=I<2097152 and 7 or 14
s,k=F(I,M(1,e)-1),H(I,e)
local O,i=R,b:vL(p)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,33)+b:iL(2147483647,(V((E(i,33)))))))
O,i=y,b:vL(T)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,12)+b:iL(2147483647,(V((E(i,12)))))))
m[Q]=b:vL(E(b:vL(s),12)+b:iL(1497155837,4294967295)+(b:iL(2797811459,12)+b:iL(2797811459,(V(12)))))
local O,i,D=J,b:vL(k),b:vL(I)
O[Q]=b:vL(E(i,41)+b:iL(1201243958,4294967295)+(b:iL(3093723338,D)+b:iL(3093723338,(V(D)))))
Q-=1
end
elseif q>=55
 if 
q<56
 n[m[Q]]()
elseif q==57
 n[R[Q]]=n[y[Q]]<=n[m[Q]]
else Q=if n[R[Q]]<=y[Q]then m[Q]else Q
end
elseif q~=54
 n[m[Q]]=E(n[y[Q]],n[R[Q]])
else n[R[Q]]=n[m[Q]]
end
Q+=1
end
end
if S==8
 while 
true do 
local 
q=y[Q]
if q>=49
 if 
q>=74
 if 
q<86
 if 
q<80
 if 
q>=77
 if 
q<78
 n[R[Q]]=E(n[J[Q]],A[Q])
elseif q~=79
 n[J[Q]]=n[R[Q]]*m[Q]
else n[R[Q]]=m[Q]+n[J[Q]]
end
elseif q<75
 p,I,T=m[Q],J[Q],R[Q]
e=T<2097152 and 7 or 14
s,k=F(T,M(1,e)-1),H(T,e)
local O,i=J,b:vL(I)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,66)+b:iL(2147483647,(V((E(i,66)))))))
O,i=m,b:vL(p)
O[Q]=b:vL(b:iL(4140372409,i)+b:iL(4140372409,57)+(b:iL(309189774,(F(57,i)))+b:iL(154594888,(E(i,57)))))
R[Q]=b:vL(E(b:vL(s),58)+b:iL(1025754316,4294967295)+(b:iL(3269212980,58)+b:iL(3269212980,(V(58)))))
local O,i,D=y,b:vL(k),b:vL(I)
O[Q]=b:vL(E(i,52)+b:iL(893018589,4294967295)+(b:iL(3401948707,D)+b:iL(3401948707,(V(D)))))
Q-=1
elseif q~=76
 Q=if R[Q]<n[J[Q]]then m[Q]else Q
else p,I,T=R[Q],m[Q],J[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local O,i=J,b:vL(T)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,112)+b:iL(2147483647,(V((E(i,112)))))))
m[Q]=b:vL(E(b:vL(I),123)+b:iL(1489073125,4294967295)+(b:iL(2805894171,123)+b:iL(2805894171,(V(123)))))
local O,D,U=R,b:vL(s),b:vL(Q)
O[Q]=b:vL(E(D,95)+b:iL(507207270,4294967295)+(b:iL(3787760026,U)+b:iL(3787760026,(V(U)))))
U,i,O=y,b:vL(k),b:vL(T)
U[Q]=b:vL(b:iL(607938432,4294967295)+b:iL(1090111384,(V((E(i,91)))))+(b:iL(1090111385,(E(i,91)))+(b:iL(2596917480,O)+b:iL(2596917480,(V(O))))))
Q-=1
end
elseif q<83
 if 
q>=81
 if 
q==82
 p,I,T=R[Q],m[Q],J[Q]
e=I<2097152 and 7 or 14
s,k=F(I,M(1,e)-1),H(I,e)
local O,i,D=J,b:vL(T),b:vL(Q)
O[Q]=b:vL(E(i,108)+b:iL(3348840705,4294967295)+(b:iL(946126591,D)+b:iL(946126591,(V(D)))))
D,i,O=m,b:vL(s),b:vL(e)
D[Q]=b:vL(E(i,37)+b:iL(1680051193,4294967295)+(b:iL(2614916103,O)+b:iL(2614916103,(V(O)))))
i,O=R,b:vL(p)
i[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,O)+(b:iL(2147483648,100)+b:iL(2147483647,(V((E(O,100)))))))
i,O=y,b:vL(k)
i[Q]=b:vL(b:iL(2017251715,O)+b:iL(4294967295,43)+(b:iL(2,(_(43,O)))+(b:iL(2277715580,4294967295)+b:iL(2017251716,(V(O))))))
Q-=1
else n[J[Q]]=n[R[Q]]<=n[m[Q]]
end
else local 
O,i,D=m[Q],n[J[Q]],x[Q]
local U,v=F(i,4294967295),F(D,4294967295)
local i,D,P,bv=F(U,65535),H(U,16),F(v,65535),H(v,16)
n[O]=F(i*P+M(F(i*bv+D*P,65535),16),4294967295)%4294967296
end
elseif q<84
 n[J[Q]]=n[R[Q]]
elseif q~=85
 n[m[Q]]=M(n[R[Q]],J[Q])
else n[J[Q]]=m[Q]*n[R[Q]]
end
elseif q<92
 if 
q<89
 if 
q>=87
 if 
q==88
 n[R[Q]]=not n[m[Q]]
else n[J[Q]]=_(n[m[Q]],n[R[Q]])
end
else n[m[Q]]=n[R[Q]]-n[J[Q]]
end
elseif q>=90
 if 
q==91
 n[R[Q]]=m[Q]
n[R[Q+1]]=m[Q+1]
Q+=1
else n[J[Q]]=n[R[Q]]-A[Q]
end
else n[m[Q]]=H(n[R[Q]],J[Q])
end
elseif q>=95
 if 
q<97
 if 
q==96
 n[R[Q]]=n[m[Q]][J[Q]]
else n[R[Q]]=Z(J[Q])
end
elseif q==98
 n[m[Q]]=_(n[J[Q]],x[Q])
else n[J[Q]]=n[m[Q]]%4294967296
end
elseif q>=93
 if 
q~=94
 p,I,T=R[Q],J[Q],m[Q]
e=p+I
n[p]=G(n[p](j(n,p+1,e)))
else n[J[Q]]=n[R[Q]]>=m[Q]
end
else n[R[Q]]=n[J[Q]](j(n[m[Q]],1,n[m[Q]][f]))
end
elseif q<61
 if 
q>=55
 if 
q>=58
 if 
q>=59
 if 
q~=60
 p=z[m[Q]]
n[J[Q]]=p[4][p[7]]
else Q=n[m[Q]]
end
else n[J[Q]]=n[R[Q]]
n[R[Q+1]]=m[Q+1]
Q+=1
end
elseif q>=56
 if 
q==57
 n[m[Q]]=F(n[R[Q]],n[J[Q]])
else p,I,T=m[Q],R[Q],J[Q]
e=n[p]
w(n,p+1,p+I,T+1,e)
end
else n[J[Q]]=n[m[Q]]*x[Q]
end
elseif q>=52
 if 
q<53
 n[R[Q]]=_(A[Q],n[J[Q]])
elseif q~=54
 n[J[Q]]=b[R[Q]]
else p,I,T=R[Q],m[Q],J[Q]
e=T<16384 and 7 or(T<2097152 and 14 or 21)
s,k=F(T,M(1,e)-1),H(T,e)
local O,i,D=J,b:vL(s),b:vL(e)
O[Q]=b:vL(E(i,19)+b:iL(923508718,4294967295)+(b:iL(3371458578,D)+b:iL(3371458578,(V(D)))))
i,O,D=m,b:vL(I),b:vL(s)
i[Q]=b:vL(E(O,108)+b:iL(2147483648,O)+(b:iL(2147483648,D)+b:iL(2147483648,(E(O,D)))))
R[Q]=b:vL(E(b:vL(p),85)+b:iL(3948517577,4294967295)+(b:iL(346449719,85)+b:iL(346449719,(V(85)))))
O,D=y,b:vL(k)
O[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,D)+(b:iL(2147483648,52)+b:iL(2147483647,(V((E(D,52)))))))
Q-=1
end
elseif q>=50
 if 
q~=51
 if 
o
 for 
O in K,o,nil do 
if 
o
 local 
i=o[O]
if i
 i[4]=i
i[6]=n[O]
i[7]=6
o[O]=nil
end
end
end
end
return
else n[J[Q]]()
end
else p=A[Q]
local O,i=x[Q],z
local D=O and#O/2 or 0
local U,v=D>0 and{}
if U
 v=o
for P=1,D,1 do 
local 
D=(P-1)*2
local bv,uv=O[D+2],O[D+1]
if bv==2
 v=if not v
{}else v
local O,D=v[uv]
if not O
 O={[7]=uv,[4]=n}
v[uv]=O
D=O
else D=O
end
U[P]=D
elseif bv==1
 U[P]=n[uv]
elseif bv==0
 U[P]={[7]=uv,[4]=n}
elseif bv==3
 U[P]=i[uv]
end
end
else v=o
end
T=b[p[4]](b,p,U,nil)
B(T,c)
n[J[Q]]=T
I,o=U,v
end
elseif q>=67
 if 
q<70
 if 
q<68
 Q=if n[J[Q]]==R[Q]then m[Q]else Q
elseif q~=69
 p=z[m[Q]]
p[4][p[7]]=n[J[Q]]
else n[R[Q]]=n[m[Q]]<=J[Q]
end
elseif q>=72
 if 
q==73
 n[J[Q]]=F(A[Q],n[R[Q]])
else n[m[Q]]=n[J[Q]]()
end
elseif q~=71
 n[R[Q]]=E(n[J[Q]],n[m[Q]])
else n[J[Q]][n[m[Q]]]=n[R[Q]]
end
elseif q>=64
 if 
q<65
 n[J[Q]]=n[R[Q]]>=n[m[Q]]
elseif q==66
 Q=if n[J[Q]]<m[Q]then R[Q]else Q
else p=J[Q]
I,T,e=n[p],n[p+1],n[p+2]
n[p]=I(T,e)
end
elseif q<62
 n[J[Q]]=#n[m[Q]]
elseif q~=63
 n[R[Q]]=n[J[Q]][n[m[Q]]]
else n[R[Q]]=m[Q]
end
elseif q<24
 if 
q>=12
 if 
q<18
 if 
q>=15
 if 
q<16
 n[m[Q]]=b
elseif q~=17
 n[m[Q]]=-n[J[Q]]
else local 
O,i,D=u,J[Q],R[Q]
local U=O[2]
O=U[4]
local v=E(O[i],287072149)
O[i]=v
i,O=U[7],v+1
U=d(i,O)
local P,bv
if U<128
 P,bv=U,O+1
else v=d(i,O+1)
if v<128
 P,bv=U-128+v*128,O+2
else local 
uv=d(i,O+2)
if uv<128
 P,bv=U-128+(v-128)*128+uv*16384,O+3
else local 
zv=d(i,O+3)
P,bv=U-128+(v-128)*128+(uv-128)*16384+zv*2097152,O+4
end
end
end
for O=bv,bv+P-1,1 do 
Y(i,O,(E(d(i,O),D)))
end
J[Q],R[Q],m[Q],y[Q]=162,135,215,3
end
elseif q<13
 n[R[Q]]=n[J[Q]]%m[Q]
elseif q==14
 p,I,T=m[Q],J[Q],R[Q]
e,s,k=p+T-1,p+I,G(n[p](j(n,p+1,p+I)))
w(k,1,T,p,n)
else n[R[Q]]=n[m[Q]]==J[Q]
end
elseif q<21
 local 
O=Q
if q<19
 S,Q=J[O],R[O]+1
break
elseif q~=20
 n[J[O]]=n[m[O]]%n[R[O]]
else n[R[O]]=n[m[O]]==n[J[O]]
end
elseif q>=22
 if 
q==23
 Q=if n[R[Q]]then m[Q]else J[Q]
else n[R[Q]]=J[Q]-n[m[Q]]
end
else n[J[Q]]=m[Q]%n[R[Q]]
end
elseif q>=6
 if 
q<9
 if 
q<7
 n[J[Q]]=F(n[m[Q]],R[Q])
elseif q==8
 p,I,T=m[Q],J[Q],R[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local O,i,D=J,b:vL(I),b:vL(Q)
O[Q]=b:vL(E(i,120)+b:iL(349546998,4294967295)+(b:iL(3945420298,D)+b:iL(3945420298,(V(D)))))
D,O,i=m,b:vL(s),b:vL(Q)
D[Q]=b:vL(E(O,75)+b:iL(69222738,4294967295)+(b:iL(4225744558,i)+b:iL(4225744558,(V(i)))))
i,D,O=R,b:vL(T),b:vL(Q)
i[Q]=b:vL(E(D,119)+b:iL(771119158,4294967295)+(b:iL(3523848138,O)+b:iL(3523848138,(V(O)))))
D,i,O=y,b:vL(k),b:vL(s)
D[Q]=b:vL(E(i,21)+b:iL(1805222646,4294967295)+(b:iL(2489744650,O)+b:iL(2489744650,(V(O)))))
Q-=1
else n[J[Q]]=E(A[Q],n[R[Q]])
end
elseif q>=10
 if 
q~=11
 n[m[Q]][R[Q]]=n[J[Q]]
else local 
O,i,D=J[Q],x[Q],A[Q]
local U,v=F(i,4294967295),F(D,4294967295)
local i,D,P,bv=F(U,65535),H(U,16),F(v,65535),H(v,16)
n[O]=F(i*P+M(F(i*bv+D*P,65535),16),4294967295)%4294967296
end
else n[m[Q]]=n[J[Q]]-R[Q]
end
elseif q>=3
 if 
q>=4
 if 
q~=5
 Q=if n[R[Q]]<=J[Q]then m[Q]else Q
else n[R[Q]]=n[J[Q]]+m[Q]
end
end
elseif q<1
 n[m[Q]]=n[R[Q]](n[J[Q]])
elseif q==2
 n[J[Q]]=F(n[m[Q]],x[Q])
else p=J[Q]+1
for O=1,R[Q],1 do 
I=F(E(m[Q],O),127)
J[p]=E(J[p],I)
m[p]=E(m[p],I)
R[p]=E(R[p],I)
y[p]=E(y[p],I)
p+=1
end
y[Q]=3
end
elseif q<36
 if 
q>=30
 if 
q>=33
 if 
q>=34
 if 
q~=35
 for 
O=J[Q],m[Q],1 do 
n[O]=nil
end
else n[m[Q]]=n[J[Q]]-R[Q]
n[J[Q+1]]=n[R[Q+1]]*m[Q+1]
n[J[Q+2]]=n[R[Q+2]]+n[m[Q+2]]
Q+=2
end
else local 
O,i,D=m[Q],n[R[Q]],n[J[Q]]
local U,v=F(i,4294967295),F(D,4294967295)
local i,D,P,bv=F(U,65535),H(U,16),F(v,65535),H(v,16)
n[O]=F(i*P+M(F(i*bv+D*P,65535),16),4294967295)%4294967296
end
elseif q<31
 local 
O,i,D=R[Q],J[Q],n[m[Q]]
local U,v=F(i,4294967295),F(D,4294967295)
local i,D,P,bv=F(U,65535),H(U,16),F(v,65535),H(v,16)
n[O]=F(i*P+M(F(i*bv+D*P,65535),16),4294967295)%4294967296
elseif q==32
 n[R[Q]]=_(X[Q],A[Q])
else if 
o
 for 
O in K,o,nil do 
if 
o
 local 
i=o[O]
if i
 i[4]=i
i[6]=n[O]
i[7]=6
o[O]=nil
end
end
end
end
return n[m[Q]]
end
elseif q<27
 if 
q<25
 n[J[Q]]=n[R[Q]]+n[m[Q]]
elseif q~=26
 local 
O,i,D=R[Q],X[Q],n[m[Q]]
local U,v=F(i,4294967295),F(D,4294967295)
local i,D,P,bv=F(U,65535),H(U,16),F(v,65535),H(v,16)
n[O]=F(i*P+M(F(i*bv+D*P,65535),16),4294967295)%4294967296
else n[R[Q]]=F(A[Q],X[Q])
end
elseif q<28
 n[R[Q]]={}
elseif q~=29
 n[R[Q]]=X[Q]
else n[R[Q]]=V(n[J[Q]])
end
elseif q>=42
 if 
q>=45
 if 
q>=47
 if 
q==48
 n[J[Q]]=n[m[Q]]~=R[Q]
else n[m[Q]]=n[R[Q]]<J[Q]
end
elseif q==46
 n[R[Q]]=n[J[Q]]>=A[Q]
else n[J[Q]]=n[R[Q]]
n[J[Q+1]]=n[R[Q+1]]
Q+=1
end
elseif q<43
 Q=R[Q]
elseif q~=44
 n[m[Q]]=n[R[Q]](X[Q])
else n[R[Q]]=n[J[Q]]*n[m[Q]]
end
elseif q<39
 if 
q>=37
 if 
q~=38
 p,I,T=J[Q],m[Q],R[Q]
e=p<2097152 and 7 or 14
s,k=F(p,M(1,e)-1),H(p,e)
local O,i=J,b:vL(s)
O[Q]=b:vL(E(i,94)+b:iL(2160444118,4294967295)+(b:iL(2134523178,i)+b:iL(2134523178,(V(i)))))
m[Q]=b:vL(E(b:vL(I),127)+b:iL(913517489,4294967295)+(b:iL(3381449807,127)+b:iL(3381449807,(V(127)))))
local O,D,U=R,b:vL(T),b:vL(p)
O[Q]=b:vL(E(D,48)+b:iL(1626625794,4294967295)+(b:iL(2668341502,U)+b:iL(2668341502,(V(U)))))
U,i=y,b:vL(k)
U[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,i)+(b:iL(2147483648,81)+b:iL(2147483647,(V((E(i,81)))))))
Q-=1
else n[m[Q]]=n[J[Q]]+x[Q]
end
else p=z[R[Q]]
n[J[Q]]=p[4][p[7]][n[m[Q]]]
end
elseif q>=40
 if 
q~=41
 n[J[Q]]=n[m[Q]]>R[Q]
else n[R[Q]]=b[X[Q]]
end
else n[R[Q]]=A[Q]+X[Q]
end
Q+=1
end
end
if S==140
 while 
true do 
local 
q=R[Q]
if q<47
 if 
q<23
 if 
q<11
 if 
q<5
 if 
q>=2
 if 
q<3
 n[J[Q]][y[Q]]=n[m[Q]]
elseif q==4
 n[m[Q]]=n[J[Q]]%4294967296
else L,C,W,r=r[9],r[8],r[5],r[6]
end
elseif q==1
 b[X[Q]]=t[Q]
else p,I,T=J[Q],m[Q],y[Q]
e,s,k=p+T-1,p+I,G(n[p](j(n,p+1,p+I)))
w(k,1,T,p,n)
end
elseif q<8
 if 
q<6
 p=z[y[Q]]
p[4][p[7]]=n[J[Q]]
elseif q==7
 n[m[Q]]=n
else p,I,T=y[Q],J[Q],m[Q]
e=T<16384 and 7 or(T<2097152 and 14 or 21)
s,k=F(T,M(1,e)-1),H(T,e)
local x,S=m,b:vL(s)
x[Q]=b:vL(b:iL(1722109500,S)+b:iL(1722109500,59)+(b:iL(850748296,(_(S,59)))+b:iL(1722109501,(E(S,59)))))
local O,i,D=J,b:vL(I),b:vL(k)
O[Q]=b:vL(E(i,108)+b:iL(639356291,4294967295)+(b:iL(3655611005,D)+b:iL(3655611005,(V(D)))))
D,i,O=y,b:vL(p),b:vL(T)
D[Q]=b:vL(E(i,88)+b:iL(2179721552,4294967295)+(b:iL(2115245744,O)+b:iL(2115245744,(V(O)))))
D,x,S=R,b:vL(k),b:vL(e)
D[Q]=b:vL(E(x,118)+b:iL(1055568219,4294967295)+(b:iL(3239399077,S)+b:iL(3239399077,(V(S)))))
Q-=1
end
elseif q<9
 local 
x,S,O=u,y[Q],m[Q]
local u=x[2]
x=u[4]
local i=E(x[S],287072149)
x[S]=i
S,x=u[7],i+1
u=d(S,x)
local D,U
if u<128
 D,U=u,x+1
else i=d(S,x+1)
if i<128
 D,U=u-128+i*128,x+2
else local 
v=d(S,x+2)
if v<128
 D,U=u-128+(i-128)*128+v*16384,x+3
else local 
P=d(S,x+3)
D,U=u-128+(i-128)*128+(v-128)*16384+P*2097152,x+4
end
end
end
for u=U,U+D-1,1 do 
Y(S,u,(E(d(S,u),O)))
end
y[Q],m[Q],J[Q],R[Q]=134,126,172,32
elseif q==10
 n[J[Q]]=n[m[Q]]%n[y[Q]]
else p,I,T=J[Q],y[Q],m[Q]
e=p<16384 and 7 or(p<2097152 and 14 or 21)
s,k=F(p,M(1,e)-1),H(p,e)
local u,d,Y=m,b:vL(T),b:vL(p)
u[Q]=b:vL(E(d,18)+b:iL(29169253,4294967295)+(b:iL(4265798043,Y)+b:iL(4265798043,(V(Y)))))
local u,Y,x,S=J,b:vL(s),b:vL(T),b:vL(p)
u[Q]=b:vL(E(Y,3)+b:iL(2147483648,x)+(b:iL(2147483648,S)+b:iL(2147483648,(E(x,S)))))
x,u,S=y,b:vL(I),b:vL(Q)
x[Q]=b:vL(E(u,122)+b:iL(488471930,4294967295)+(b:iL(3806495366,S)+b:iL(3806495366,(V(S)))))
Y,d,u=R,b:vL(k),b:vL(I)
Y[Q]=b:vL(E(d,119)+b:iL(241447377,4294967295)+(b:iL(4053519919,u)+b:iL(4053519919,(V(u)))))
Q-=1
end
elseif q<17
 if 
q>=14
 if 
q>=15
 if 
q==16
 if 
o
 for 
u in K,o,nil do 
if 
o
 local 
d=o[u]
if d
 d[4]=d
d[6]=n[u]
d[7]=6
o[u]=nil
end
end
end
end
return
else n[J[Q]]=G(n[m[Q]](n[y[Q]]))
end
else n[J[Q]](n[m[Q]])
n[m[Q+1]]=n[J[Q+1]][y[Q+1]]
n[J[Q+2]](n[m[Q+2]])
n[m[Q+3]]=n[J[Q+3]][y[Q+3]]
Q+=3
end
elseif q<12
 if 
o
 for 
u in K,o,nil do 
if 
o
 local 
d=o[u]
if d
 d[4]=d
d[6]=n[u]
d[7]=6
o[u]=nil
end
end
end
end
return t[Q]
elseif q~=13
 n[J[Q]]=G(n[m[Q]](j(n[y[Q]],1,n[y[Q]][f])))
else local 
u,d,Y=y[Q],X[Q],n[m[Q]]
local x,S=F(d,4294967295),F(Y,4294967295)
local d,Y,O,i=F(x,65535),H(x,16),F(S,65535),H(S,16)
n[u]=F(d*O+M(F(d*i+Y*O,65535),16),4294967295)%4294967296
end
elseif q<20
 if 
q>=18
 if 
q==19
 n[y[Q]]=_(n[m[Q]],n[J[Q]])
else n[m[Q]](n[J[Q]],t[Q])
end
else n[J[Q]](n[m[Q]])
n[m[Q+1]]=n[J[Q+1]][y[Q+1]]
n[J[Q+2]](n[m[Q+2]])
Q+=2
end
elseif q>=21
 if 
q==22
 n[y[Q]]=m[Q]
else n[J[Q]]=E(n[m[Q]],n[y[Q]])
end
else local 
u,d,Y=y[Q],m[Q],n[J[Q]]
local x,S=F(d,4294967295),F(Y,4294967295)
local d,Y,O,i=F(x,65535),H(x,16),F(S,65535),H(S,16)
n[u]=F(d*O+M(F(d*i+Y*O,65535),16),4294967295)%4294967296
end
elseif q<35
 if 
q>=29
 if 
q<32
 if 
q>=30
 if 
q==31
 n[y[Q]]=n[J[Q]][A[Q]]
else n[y[Q]]=m[Q]
n[y[Q+1]]=m[Q+1]
Q+=1
end
else n[J[Q]]=F(n[y[Q]],n[m[Q]])
end
elseif not(q<33)then if 
q==34
 n[y[Q]]=n[J[Q]]<m[Q]
else p=X[Q]
local u,d=A[Q],z
local Y=u and#u/2 or 0
local x,S=Y>0 and{}
if x
 S=o
for O=1,Y,1 do 
local 
Y=(O-1)*2
local i,D=u[Y+2],u[Y+1]
if i==2
 S=if not S
{}else S
local u,Y=S[D]
if not u
 u={[7]=D,[4]=n}
S[D]=u
Y=u
else Y=u
end
x[O]=Y
elseif i==1
 x[O]=n[D]
elseif i==0
 x[O]={[7]=D,[4]=n}
elseif i==3
 x[O]=d[D]
end
end
else S=o
end
T=b[p[4]](b,p,x,nil)
B(T,c)
n[y[Q]]=T
I,o=x,S
end
end
elseif q<26
 if 
q<24
 n[y[Q]]=n[J[Q]]<=m[Q]
elseif q~=25
 local 
u,B,d=m[Q],n[y[Q]],n[J[Q]]
local Y,x=F(B,4294967295),F(d,4294967295)
local B,d,S,O=F(Y,65535),H(Y,16),F(x,65535),H(x,16)
n[u]=F(B*S+M(F(B*O+d*S,65535),16),4294967295)%4294967296
else n[y[Q]]=c[A[Q]]
end
elseif q>=27
 if 
q==28
 r,p,I={[8]=C,[6]=r,[5]=W,[9]=L},m[Q],g(l)
I(b,n[p],n[p+1],n[p+2])
Q,L=J[Q],I
else p,I,T=m[Q],y[Q],J[Q]
e,s=p+T-1,p+I
k=n[s]
a=k[f]
h=I+a-1
k[f]=h
w(k,1,a,I,k)
w(n,p+1,s-1,1,k)
s=G(n[p](j(k,1,k[f])))
w(s,1,T,p,n)
end
else n[y[Q]]=n[m[Q]](n[J[Q]])
end
elseif q<41
 if 
q>=38
 if 
q>=39
 if 
q~=40
 n[m[Q]]=n[y[Q]][n[J[Q]]]
else n[m[Q]]=b[y[Q]]
end
else Q=n[y[Q]]
end
elseif q<36
 n[y[Q]]=n[J[Q]]-n[m[Q]]
elseif q==37
 n[m[Q]][J[Q]]=t[Q]
else n[J[Q]]=F(n[y[Q]],m[Q])
end
elseif q>=44
 if 
q<45
 p,I,T=J[Q],y[Q],m[Q]
e=T<2097152 and 7 or 14
s,k=F(T,M(1,e)-1),H(T,e)
local u,B=m,b:vL(s)
u[Q]=b:vL(E(B,3)+b:iL(1355300923,4294967295)+(b:iL(2939666373,B)+b:iL(2939666373,(V(B)))))
local u,d,Y=J,b:vL(p),b:vL(s)
u[Q]=b:vL(E(d,13)+b:iL(273088301,4294967295)+(b:iL(4021878995,Y)+b:iL(4021878995,(V(Y)))))
d,B,Y=y,b:vL(I),b:vL(e)
d[Q]=b:vL(E(B,74)+b:iL(1774094070,4294967295)+(b:iL(2520873226,Y)+b:iL(2520873226,(V(Y)))))
B,d,u=R,b:vL(k),b:vL(s)
B[Q]=b:vL(E(d,121)+b:iL(1649884357,4294967295)+(b:iL(2645082939,u)+b:iL(2645082939,(V(u)))))
Q-=1
elseif q~=46
 local 
u=m[Q]
if o
 local 
B=o[u]
if B
 B[4]=B
B[6]=n[u]
B[7]=6
o[u]=nil
end
end
else p,I,T=J[Q],m[Q],y[Q]
e=T<16384 and 7 or(T<2097152 and 14 or 21)
s,k=F(T,M(1,e)-1),H(T,e)
local u,B,d=m,b:vL(I),b:vL(T)
u[Q]=b:vL(E(B,53)+b:iL(605869183,4294967295)+(b:iL(3689098113,d)+b:iL(3689098113,(V(d)))))
B,d=J,b:vL(p)
B[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,d)+(b:iL(2147483648,19)+b:iL(2147483647,(V((E(d,19)))))))
B,d,u=y,b:vL(s),b:vL(T)
B[Q]=b:vL(E(d,77)+b:iL(380729186,4294967295)+(b:iL(3914238110,u)+b:iL(3914238110,(V(u)))))
R[Q]=b:vL(E(b:vL(k),73)+b:iL(400206220,4294967295)+(b:iL(3894761076,73)+b:iL(3894761076,(V(73)))))
Q-=1
end
elseif q<42
 Q=J[Q]
elseif q==43
 n[J[Q]](n[m[Q]])
else p=z[J[Q]]
n[m[Q]]=p[4][p[7]]
end
elseif q>=71
 if 
q>=83
 if 
q>=89
 if 
q<92
 if 
q<90
 n[y[Q]]=n[J[Q]]()
elseif q==91
 p,I,T=J[Q],y[Q],m[Q]
e=p<2097152 and 7 or 14
s,k=F(p,M(1,e)-1),H(p,e)
local u,z=m,b:vL(T)
u[Q]=b:vL(b:iL(122948472,z)+b:iL(122948472,127)+(b:iL(4049070352,(F(127,z)))+b:iL(4172018825,(E(z,127)))))
local u,z,B=J,b:vL(s),b:vL(k)
u[Q]=b:vL(E(z,29)+b:iL(725162674,4294967295)+(b:iL(3569804622,B)+b:iL(3569804622,(V(B)))))
y[Q]=b:vL(E(b:vL(I),35)+b:iL(461449896,4294967295)+(b:iL(3833517400,35)+b:iL(3833517400,(V(35)))))
B,z=R,b:vL(k)
B[Q]=b:vL(b:iL(2147483649,4294967295)+b:iL(2147483648,z)+(b:iL(2147483648,45)+b:iL(2147483647,(V((E(z,45)))))))
Q-=1
else n[y[Q]]=n[J[Q]]+n[m[Q]]
end
elseif q>=93
 if 
q~=94
 n[m[Q]][n[y[Q]]]=n[J[Q]]
else Q=if n[J[Q]]then m[Q]else y[Q]
end
else p=y[Q]+1
for u=1,J[Q],1 do 
I=F(E(m[Q],u),127)
m[p]=E(m[p],I)
J[p]=E(J[p],I)
y[p]=E(y[p],I)
R[p]=E(R[p],I)
p+=1
end
R[Q]=32
end
elseif q>=86
 if 
q>=87
 if 
q==88
 n[m[Q]]=b[t[Q]]
else Q=if n[J[Q]]<m[Q]then y[Q]else Q
end
else p,I,T=J[Q],y[Q],m[Q]
e=p+I
s=n[e]
k=s[f]
a=I+k-1
s[f]=a
w(s,1,k,I,s)
w(n,p+1,e-1,1,s)
n[p]=G(n[p](j(s,1,s[f])))
end
elseif q>=84
 if 
q==85
 n[y[Q]]=n[m[Q]]+J[Q]
else n[y[Q]]=Z(m[Q])
end
else n[y[Q]]=m[Q]-n[J[Q]]
end
elseif q>=77
 if 
q<80
 if 
q<78
 b[A[Q]]=n[y[Q]]
elseif q~=79
 n[y[Q]]=X[Q]+A[Q]
else n[J[Q]][t[Q]]=n[m[Q]]
end
elseif q<81
 n[J[Q]]=n[y[Q]](A[Q])
elseif q==82
 n[m[Q]]=n[J[Q]][y[Q]]
else p,I,T=J[Q],m[Q],y[Q]
e=T<2097152 and 7 or 14
s,k=F(T,M(1,e)-1),H(T,e)
local u,z=m,b:vL(I)
u[Q]=b:vL(b:iL(281358121,z)+b:iL(281358121,58)+(b:iL(3732251054,(F(z,58)))+b:iL(4013609176,(E(z,58)))))
local u,Z,B=J,b:vL(p),b:vL(s)
u[Q]=b:vL(E(Z,47)+b:iL(166506910,4294967295)+(b:iL(4128460386,B)+b:iL(4128460386,(V(B)))))
B,Z,z=y,b:vL(s),b:vL(Q)
B[Q]=b:vL(E(Z,41)+b:iL(165473990,4294967295)+(b:iL(4129493306,z)+b:iL(4129493306,(V(z)))))
z,B=R,b:vL(k)
z[Q]=b:vL(b:iL(4294967295,B)+b:iL(269741780,70)+(b:iL(2,(_(70,B)))+(b:iL(4025225515,4294967295)+b:iL(269741781,(V(70))))))
Q-=1
end
elseif q<74
 if 
q>=72
 if 
q==73
 n[J[Q]]=n[y[Q]]-m[Q]
else n[y[Q]]=not n[m[Q]]
end
else Q=if n[J[Q]]<=m[Q]then y[Q]else Q
end
elseif q>=75
 if 
q~=76
 n[y[Q]]=n[J[Q]]
else p,I,T=J[Q],m[Q],y[Q]
e=p+I
n[p]=G(n[p](j(n,p+1,e)))
end
else p=J[Q]
I,T,e=n[p],n[p+1],n[p+2]
n[p]=I(T,e)
end
elseif q<59
 if 
q>=53
 if 
q<56
 if 
q<54
 b[y[Q]]=n[m[Q]]
elseif q~=55
 p,I=y[Q],n[m[Q]]
n[p+1]=I
n[p]=I[X[Q]]
else n[y[Q]]=b
end
elseif q<57
 n[y[Q]][X[Q]]=A[Q]
elseif q~=58
 n[y[Q]]=A[Q]
else if 
o
 for 
b in K,o,nil do 
if 
o
 local 
u=o[b]
if u
 u[4]=u
u[6]=n[b]
u[7]=6
o[b]=nil
end
end
end
end
return n[y[Q]]
end
elseif q<50
 if 
q<48
 Q=if m[Q]<n[J[Q]]then y[Q]else Q
elseif q~=49
 n[y[Q]]=V(n[J[Q]])
else n[y[Q]]=J[Q]*n[m[Q]]
end
elseif q<51
 n[y[Q]]()
elseif q==52
 p,I,T=J[Q],y[Q],m[Q]
e,s=n[p],p+I
k=n[s]
w(n,p+1,s-1,T+1,e)
w(k,1,k[f],T+I,e)
else if 
o
 for 
b in K,o,nil do 
if 
o
 local 
u=o[b]
if u
 u[4]=u
u[6]=n[b]
u[7]=6
o[b]=nil
end
end
end
end
return j(n[m[Q]],1,n[m[Q]][f])
end
elseif q<65
 if 
q<62
 if 
q>=60
 if 
q~=61
 n[J[Q]]=n[m[Q]]%y[Q]
else n[y[Q]]=n[J[Q]]>=n[m[Q]]
end
else local 
b,u,z=y[Q],A[Q],X[Q]
local Z,o=F(u,4294967295),F(z,4294967295)
local u,z,K,G=F(Z,65535),H(Z,16),F(o,65535),H(o,16)
n[b]=F(u*K+M(F(u*G+z*K,65535),16),4294967295)%4294967296
end
elseif q>=63
 if 
q==64
 n[J[Q]]=A[Q]+n[y[Q]]
else n[m[Q]]=n[y[Q]]~=X[Q]
end
else n[J[Q]]=n[y[Q]]<=n[m[Q]]
end
elseif q<68
 if 
q>=66
 if 
q==67
 p,I,T,e=m[Q],L()
if I
 n[p+1]=T
n[p+2]=e
Q=J[Q]
end
else p,I,T=J[Q],m[Q],y[Q]
e=n[p]
w(n,p+1,p+I,T+1,e)
end
else n[J[Q]]=#n[m[Q]]
end
elseif q<69
 n[m[Q]][t[Q]]=J[Q]
elseif q~=70
 n[J[Q]]={}
else n[y[Q]](n[m[Q]],n[J[Q]])
end
Q+=1
end
end
end
else return 
N
end
end
end

mkdir -p /home/ubuntu/steamcmd
cd /home/ubuntu/steamcmd
curl -sqL "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz" | tar zxvf -
./steamcmd.sh +force_install_dir /home/ubuntu/pzserver +login anonymous +app_update 380870 -beta unstable validate +quit
cd /home/ubuntu/pzserver
bash start-server.sh -servername SERVERNAME &
sleep 20
kill $!

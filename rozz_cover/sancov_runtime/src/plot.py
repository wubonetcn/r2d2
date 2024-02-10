import seaborn as sns
import os
import sys
import json
import pandas as pd
import matplotlib.pyplot as plt

if len(sys.argv) != 2:
    exit()

log_dir = sys.argv[1]
plot_data = []
for path in os.listdir(log_dir):
    index = path.find("serverlog")
    if index == -1:
        continue
    name = path[:index]
    file = os.path.join(log_dir, path)
    print(file)
    start_time = 0
    with open(file) as f:
        for line in f.readlines():
            data = json.loads(line)
            if start_time == 0:
                start_time = int(data["timestamp"])
            plot_data.append([int(data["timestamp"]) - start_time, int(data["covered_num"]), name])
print(len(plot_data))
df = pd.DataFrame(plot_data, columns=['time', 'edge coverage', 'fuzzer'], dtype=int)
print(df.head(5))
g = sns.lineplot(
    data=df
    , x="time", y="edge coverage", hue="fuzzer", err_style="bars", ci=68
)


figure_fig = plt.gcf()  # 'get current figure'
figure_fig.savefig('plot.pdf',
                   format='pdf',
                   dpi=1000,
                   bbox_inches='tight',
                   pad_inches=0)

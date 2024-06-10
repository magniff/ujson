import json
import time

with open('data.json') as f:
    t0 = time.time()
    data = json.load(f)
    t1 = time.time()
    print('Time:', t1 - t0, 'seconds')

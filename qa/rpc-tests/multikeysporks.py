#!/usr/bin/env python3
# Copyright (c) 2018 The Dash Core developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

from test_framework.mininode import *
from test_framework.test_framework import BitcoinTestFramework
from test_framework.util import *
from time import *
'''
'''

class MultiKeySporkTest(BitcoinTestFramework):
    def __init__(self):
        super().__init__()
        self.num_nodes = 3
        self.setup_clean_chain = True
        self.is_network_split = False

    def setup_network(self):
        self.nodes = []

        # secret(base58): 931wyuRNVYvhg18Uu9bky5Qg1z4QbxaJ7fefNBzjBPiLRqcd33F
        # keyid(hex): 60f0f57f71f0081f1aacdd8432340a33a526f91b
        # address(base58): yNsMZhEhYqv14TgdYb1NS2UmNZjE8FSJxa
        # secret(base58): 91vbXGMSWKGHom62986XtL1q2mQDA12ngcuUNNe5NfMSj44j7g3
        # keyid(hex): 43dff2b09de2f904f688ec14ee6899087b889ad0
        # address(base58): yfLSXFfipnkgYioD6L8aUNyfRgEBuJv48h
        # secret(base58): 92bxUjPT5AhgXuXJwfGGXqhomY2SdQ55MYjXyx9DZNxCABCSsRH
        # keyid(hex): d9aa5fa00cce99101a4044e65dc544d1579890de
        # address(base58): ygcG5S2pQz2U1UAaHvU6EznKZW7yapKMA7
        # secret(base58): 934yPXiVGf4RCY2qTs2Bt5k3TEtAiAg12sMxCt8yVWbSU7p3fuD
        # keyid(hex): 0b23935ce0bea3b997a334f6fa276c9fa17687b2
        # address(base58): ycbRQWbovrhQMTuxg9p4LAuW5SCMAKqPrn
        # secret(base58): 92Cxwia363Wg2qGF1fE5z4GKi8u7r1nrWQXdtsj2ACZqaDPSihD
        # keyid(hex): 1d1098b2b1f759b678a0a7a098637a9b898adcac
        # address(base58): yc5TGfcHYoLCrcbVy4umsiDjsYUn39vLui
        # secret(base58): 92CKXK5dbysc4zQnNsvzgDpiBGgo67VPsRDAFsF4gGwbiZpgLMv
        # keyid(hex): 0e9b6bdc15dc3afb63d1ffc68cc5534a4caaeaa6
        # address(base58): ybY28sZzX3Jy3UNcZ72MNubcVEQXcAN3Vh
        # secret(base58): 91tDdZPR5gfCNfkVN8Pym1YyaaMHhf9MyVS12LRuN2MoAgSeDcE
        # keyid(hex): 99a001480e0332d1a0d30078bde4d6fe3a178cfa
        # address(base58): yjADbqTD649DaJnRGZYrXW1fnfQ4ciDcXH
        # secret(base58): 928dGxqesKbqKpfAJXRnGd4gy3GtbtfM1WHTQBisvWDVJZ1Vxyi
        # keyid(hex): 3ae21cfc6017c65e80d7ec45310c22f6dc705bdd
        # address(base58): ygVsj29xcd1B3iMbs6A2CY9mscC8cBXEZX
        # secret(base58): 91ynY1Q7iaXSbuNq9nQ2u6K2i2ryX3em5wEKEofXTjhsrDL2jY8
        # keyid(hex): 551a3c676574d355fc3c314955f3b440cf57b245
        # address(base58): ySfy7vpGjiG27PSr8XKCKr4y3mY1D9U76y
        # secret(base58): 92ffBcNsSLyEKT5CmqGMEmYHJBUTHoGHjFy9Y3XNPSLbxi4DVWD
        # keyid(hex): 8df696c8d4e69a898ce7d599559e32268b28ccbb
        # address(base58): ydSRnFKrboF3L544AWQfG79mcXMC7ZtShL
        # secret(base58): 92cDaZ9GGSLnJsaEbCDewrXA3RcmDCG9MZAeJZTmMnpgLQAsyHr
        # keyid(hex): 92af1b1ebe0107dcc648aa150905e1229777225d
        # address(base58): yUotyrzs4H9DJ7K3Y7ccgeAUh442FEkrKw
        # secret(base58): 92PGUYidWoq9qBoyEEAeYQ12ZHWBCR5YusLenpa7vUGFXPfgqXx
        # keyid(hex): 093a9963ac777502251dd98c22ea2c71aaa2fee3
        # address(base58): yh6yH8LJ4eTFWAWVNdCDaQa53nXpddvmZp
        # secret(base58): 91tbFNAjdSeUYZAG3QhF91Zh1LGgZcFpq8zrGEH1MHKycFLtDrL
        # keyid(hex): 5fcfae286651d85e6ad2eff5a38b148d806f9beb
        # address(base58): yhoDr8n8DzfLR7VQGcLqpmq29WJ5Z2eYNi
        # secret(base58): 92EMsUxGgk1372edF7iNNzQSN3iLsRkUSmZzQKGwLd4CrC3YUUH
        # keyid(hex): 4287e28642ebfd3d48e86888a6bb6e7a9af22910
        # address(base58): yMnuur7LEVgrYFczsNfx8W61eAgGYAmTVG
        # secret(base58): 92VRomWTLSRRHWR3NwbUAhrPuwfMyciLcDWR9ebJRbSFoEj2RLg
        # keyid(hex): 6d3fcee4b4b92488ed20171b61c2951ea51cbeb4
        # address(base58): yco8DwPVN6Wgk1nCmakkLYCGzXy9fgu5zC
        # secret(base58): 92swaJmVB2c4kLvKt7CihNneE7MxfQnv7eRV248F7t4CmVWsaLe
        # keyid(hex): ab87f6995077f4bdd6233bc2992611674d7c6594
        # address(base58): yZr6Rw8GiZwD6GMdC2fuP7c8qkFQPgWVRX
        # secret(base58): 923NQ12YSeVs4RfaCiCqTE9iNAD4dobSmuEhnV7x6q7styQYfKb
        # keyid(hex): 2318b0dab38d676aae483ab0f55f2d839a1031b2
        # address(base58): ycZduSBR8W8Gpb9ec1AocYF1c4Q3E6wjeM
        # secret(base58): 92tMn4LCFGTthXpdiygJGjQbJYmYY78TiXHFWYTUu9uZiLnHn8W
        # keyid(hex): ed73329177896a951b0eb30dd962d3e661c0476e
        # address(base58): yWNZ8gibbLMAyuXX4i1ZNaxmADW59Rz3zJ
        # secret(base58): 92RgZKa9TYVRJZmj4Nme3jBRn77rD2BmQNJZJkWmN67UVpRXWjW
        # keyid(hex): a7845c35a6b608c7c3a9a9388e06370fa3f5d32b
        # address(base58): yQKBpf5bCZ3ctmYKFEPqdeLTg4SwuciMo7
        # secret(base58): 92mNuCHJ2kWcxGkAus1P6UDPHPFZDPNTzMMf72YujjXtdmbiUCk
        # keyid(hex): dfe5fcd9bee0b871ad221a4cf3532c2848314701
        # address(base58): yLSCowJMnKfrmniiX9RD2bxvbHwvzgyKvQ
        # secret(base58): 92cyrqe2iYqw6ni5MQvBAcSQNAAApmeToeA7qzawEcxLDHufGK3
        # keyid(hex): 088f9e1ee1a06e0b3fed714beee97e90c035ed76
        # address(base58): yXAGkUbnEtEeY2cdZnt9zzD4mCnmLeCZfU
        # secret(base58): 93UMgtWGffnog5yYyyiSwzHCP7FX8wmURQa19f7oT95JQpLu3LN
        # keyid(hex): 98396e03c6e20e44ce776e07e92cb533cb450e7f
        # address(base58): yXuFmFBfipN4pDWbiQLLNAaT2HcTGLLAkJ


        self.nodes.append(start_node(0, self.options.tmpdir,
                                     ["-debug", "-sporkkey=931wyuRNVYvhg18Uu9bky5Qg1z4QbxaJ7fefNBzjBPiLRqcd33F",
                                      "-sporkaddr=ygcG5S2pQz2U1UAaHvU6EznKZW7yapKMA7:yfLSXFfipnkgYioD6L8aUNyfRgEBuJv48h:yNsMZhEhYqv14TgdYb1NS2UmNZjE8FSJxa",
                                      "-minsporkkeys=2"]))
        self.nodes.append(start_node(1, self.options.tmpdir,
                                     ["-debug", "-sporkkey=91vbXGMSWKGHom62986XtL1q2mQDA12ngcuUNNe5NfMSj44j7g3",
                                      "-sporkaddr=ygcG5S2pQz2U1UAaHvU6EznKZW7yapKMA7:yfLSXFfipnkgYioD6L8aUNyfRgEBuJv48h:yNsMZhEhYqv14TgdYb1NS2UmNZjE8FSJxa",
                                      "-minsporkkeys=2"]))
        self.nodes.append(start_node(2, self.options.tmpdir,
                                     ["-debug",
                                      "-sporkaddr=ygcG5S2pQz2U1UAaHvU6EznKZW7yapKMA7:yfLSXFfipnkgYioD6L8aUNyfRgEBuJv48h:yNsMZhEhYqv14TgdYb1NS2UmNZjE8FSJxa",
                                      "-minsporkkeys=2"]))
        # connect nodes at start
        connect_nodes(self.nodes[0], 1)
        connect_nodes(self.nodes[0], 2)
        connect_nodes(self.nodes[1], 2)

    def get_test_spork_state(self, node):
        info = node.spork('active')
        # use InstantSend spork for tests
        return info['SPORK_2_INSTANTSEND_ENABLED']

    def set_test_spork_state(self, node, state):
        if state:
            value = 0
        else:
            value = 4070908800
        # use InstantSend spork for tests
        node.spork('SPORK_2_INSTANTSEND_ENABLED', value)

    def wait_for_test_spork_state(self, node, state):
        start = time()
        got_state = False
        while True:
            if self.get_test_spork_state(node) == state:
                got_state = True
                break
            if time() > start + 10:
                break
            sleep(0.1)
        return got_state

    def run_test(self):
        # check test spork default state
        for node in self.nodes:
            assert(self.get_test_spork_state(node))

        # first signer turns off spork
        self.set_test_spork_state(self.nodes[0], False)
        # spork change requires at least 2 signers
        for node in self.nodes:
            assert(not self.wait_for_test_spork_state(node, False))

        # second signer turns off spork
        self.set_test_spork_state(self.nodes[1], False)
        # now spork state is changed
        for node in self.nodes:
            assert(self.wait_for_test_spork_state(node, False))



if __name__ == '__main__':
    MultiKeySporkTest().main()

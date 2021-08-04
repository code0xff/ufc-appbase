CREATE TABLE `ufc`.`tm_tx` (
    `tm_tx_id`   bigint(20) NOT NULL AUTO_INCREMENT,
    `txhash`     varchar(100) NOT NULL,
    `height`     varchar(100) NOT NULL,
    `gas_wanted` varchar(100) NULL,
    `gas_used`   varchar(100) NULL,
    `raw_log`    text DEFAULT NULL,
    `timestamp`  varchar(100) NULL,
    PRIMARY KEY (`tm_tx_id`),
    UNIQUE KEY `tm_tx_UN` (`txhash`),
    KEY `tm_tx_height_IDX` (`height`) USING BTREE,
    KEY `tm_tx_txhash_IDX` (`txhash`) USING BTREE
) ENGINE=InnoDB AUTO_INCREMENT=1 DEFAULT CHARSET=utf8mb4;
